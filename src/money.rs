/* fbarcalc: find account maximum value
 * Copyright 2026 Mario Finelli
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Money handling
//!
//! Amounts are represented as a whole number of cents (`i64`) so that
//! entering, adding and comparing them is always exact. Floating point is
//! only used for the final currency conversion, where the result is rounded
//! back to cents exactly once.

use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseAmountError {
    /// The input isn't a plain decimal number.
    Invalid,
    /// The input is a number but is too large to represent.
    OutOfRange,
}

impl fmt::Display for ParseAmountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseAmountError::Invalid => write!(f, "invalid amount"),
            ParseAmountError::OutOfRange => write!(f, "amount out of range"),
        }
    }
}

impl Error for ParseAmountError {}

/// Parse a decimal amount (e.g. `12.34`, `-5`, `+0.5`, `.5`) into cents.
///
/// Digits beyond the second decimal place are truncated towards zero, so
/// `12.345` is 1234 cents and `-12.345` is -1234 cents. Only an optional
/// sign, digits, and a single optional decimal point are accepted: there is
/// no exponent notation, thousands separator, `nan`, or `inf`.
pub fn parse_amount(input: &str) -> Result<i64, ParseAmountError> {
    let s = input.trim();
    let (negative, unsigned) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };

    let (whole, frac) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if (whole.is_empty() && frac.is_empty())
        || !all_digits(whole)
        || !all_digits(frac)
    {
        return Err(ParseAmountError::Invalid);
    }

    // The input is only digits at this point so a parse failure can only
    // mean that the number doesn't fit.
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| ParseAmountError::OutOfRange)?
    };

    let digit =
        |i: usize| frac.as_bytes().get(i).map_or(0, |b| i64::from(b - b'0'));
    let fraction = digit(0) * 10 + digit(1);

    let cents = whole
        .checked_mul(100)
        .and_then(|c| c.checked_add(fraction))
        .ok_or(ParseAmountError::OutOfRange)?;

    Ok(if negative { -cents } else { cents })
}

/// Like [`parse_amount`] but a blank input is `None`. This is how the user
/// says that there are no more amounts.
pub fn parse_optional_amount(
    input: &str,
) -> Result<Option<i64>, ParseAmountError> {
    if input.trim().is_empty() {
        Ok(None)
    } else {
        parse_amount(input).map(Some)
    }
}

/// Format an amount of cents with its currency symbol and a comma between
/// each group of three digits, e.g. `$1,234.56`. The sign goes in front of the
/// symbol: `-$1,234.56`.
///
/// The commas are only for showing amounts: [`parse_amount`] doesn't accept
/// them because in many places a comma is the decimal separator and `1,234`
/// would be ambiguous.
pub fn format_money(symbol: &str, cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!(
        "{}{}{}.{:02}",
        sign,
        symbol,
        group_thousands(abs / 100),
        abs % 100
    )
}

/// Write a number with a comma between each group of three digits.
fn group_thousands(n: u64) -> String {
    let digits: Vec<char> = n.to_string().chars().collect();

    // the groups are counted from the end: the first one can be shorter
    digits
        .rchunks(3)
        .rev()
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(",")
}

/// Convert an amount of cents using an exchange rate, rounding to the nearest
/// cent.
pub fn convert_cents(cents: i64, rate: f64) -> i64 {
    (cents as f64 * rate).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_amounts() {
        assert_eq!(parse_amount("0"), Ok(0));
        assert_eq!(parse_amount("5"), Ok(500));
        assert_eq!(parse_amount("12.34"), Ok(1234));
        assert_eq!(parse_amount("12.3"), Ok(1230));
        assert_eq!(parse_amount("0.05"), Ok(5));
    }

    #[test]
    fn parse_is_exact_for_values_that_floats_cannot_represent() {
        // these all used to be shaved down a cent by the floating point
        // implementation (e.g. 8.2 -> 8.19)
        assert_eq!(parse_amount("8.2"), Ok(820));
        assert_eq!(parse_amount("19.99"), Ok(1999));
        assert_eq!(parse_amount("1.15"), Ok(115));
        assert_eq!(parse_amount("0.29"), Ok(29));
        assert_eq!(parse_amount("4.35"), Ok(435));
        assert_eq!(parse_amount("0.57"), Ok(57));
    }

    #[test]
    fn parse_truncates_extra_decimals_towards_zero() {
        assert_eq!(parse_amount("12.345"), Ok(1234));
        assert_eq!(parse_amount("12.349999"), Ok(1234));
        assert_eq!(parse_amount("-12.345"), Ok(-1234));
        assert_eq!(parse_amount("0.001"), Ok(0));
        assert_eq!(parse_amount("-0.001"), Ok(0));
    }

    #[test]
    fn parse_signs() {
        assert_eq!(parse_amount("-12.34"), Ok(-1234));
        assert_eq!(parse_amount("+12.34"), Ok(1234));
        assert_eq!(parse_amount("-0"), Ok(0));
    }

    #[test]
    fn parse_accepts_leading_and_trailing_decimal_point() {
        assert_eq!(parse_amount(".5"), Ok(50));
        assert_eq!(parse_amount("5."), Ok(500));
        assert_eq!(parse_amount("-.5"), Ok(-50));
    }

    #[test]
    fn parse_trims_whitespace() {
        assert_eq!(parse_amount("  12.34 "), Ok(1234));
    }

    #[test]
    fn parse_rejects_things_that_are_not_plain_decimals() {
        for input in [
            "", " ", ".", "-", "+", "-.", "abc", "12.3x", "x12.3", "12.3.4",
            "--5", "+-5", "- 5", "1 2", "1,234.50", "1e3", "1E3", "nan", "NaN",
            "inf", "-inf", "infinity", "0x10", "$5",
        ] {
            assert_eq!(
                parse_amount(input),
                Err(ParseAmountError::Invalid),
                "input {:?}",
                input
            );
        }
    }

    #[test]
    fn parse_range_limits() {
        // i64::MAX cents is 92233720368547758.07
        assert_eq!(parse_amount("92233720368547758.07"), Ok(i64::MAX));
        assert_eq!(
            parse_amount("92233720368547758.08"),
            Err(ParseAmountError::OutOfRange)
        );
        assert_eq!(
            parse_amount("92233720368547759"),
            Err(ParseAmountError::OutOfRange)
        );
        assert_eq!(
            parse_amount("99999999999999999999999999"),
            Err(ParseAmountError::OutOfRange)
        );
        assert_eq!(
            parse_amount("-99999999999999999999999999"),
            Err(ParseAmountError::OutOfRange)
        );
    }

    #[test]
    fn summing_many_small_amounts_is_exact() {
        // 1000 x 0.1 in f64 is 99.9999999999986
        let total: i64 = (0..1000).map(|_| parse_amount("0.10").unwrap()).sum();
        assert_eq!(total, 10_000);
    }

    #[test]
    fn format_positive_and_zero() {
        assert_eq!(format_money("$", 0), "$0.00");
        assert_eq!(format_money("$", 5), "$0.05");
        assert_eq!(format_money("$", 50), "$0.50");
        assert_eq!(format_money("€", 1234), "€12.34");
        assert_eq!(format_money("¥", 100_000_000), "¥1,000,000.00");
    }

    #[test]
    fn format_negative_puts_the_sign_before_the_symbol() {
        assert_eq!(format_money("$", -1234), "-$12.34");
        assert_eq!(format_money("£", -5), "-£0.05");
    }

    #[test]
    fn format_extremes_do_not_overflow() {
        assert_eq!(format_money("$", i64::MAX), "$92,233,720,368,547,758.07");
        assert_eq!(format_money("$", i64::MIN), "-$92,233,720,368,547,758.08");
    }

    #[test]
    fn format_groups_thousands() {
        for (cents, expected) in [
            (0, "$0.00"),
            (99, "$0.99"),
            (99_999, "$999.99"),
            (100_000, "$1,000.00"),
            (100_001, "$1,000.01"),
            (123_456, "$1,234.56"),
            (9_999_999, "$99,999.99"),
            (10_000_000, "$100,000.00"),
            (99_999_999, "$999,999.99"),
            (100_000_000, "$1,000,000.00"),
            (123_456_789, "$1,234,567.89"),
            (100_000_000_000, "$1,000,000,000.00"),
        ] {
            assert_eq!(format_money("$", cents), expected, "{}", cents);
        }
    }

    #[test]
    fn format_groups_thousands_of_negative_amounts() {
        assert_eq!(format_money("$", -99_999), "-$999.99");
        assert_eq!(format_money("$", -100_000), "-$1,000.00");
        assert_eq!(format_money("€", -123_456_789), "-€1,234,567.89");
    }

    #[test]
    fn format_never_puts_a_separator_in_the_cents_or_before_the_symbol() {
        let formatted = format_money("£", 123_456_789);

        assert!(formatted.starts_with('£'), "{}", formatted);
        assert!(formatted.ends_with(".89"), "{}", formatted);
        assert_eq!(formatted.matches(',').count(), 2, "{}", formatted);
    }

    #[test]
    fn format_round_trips_through_parse_without_the_separators() {
        for cents in
            [0, 1, 99, 100, 101, 123_456, 123_456_789, -1, -100, -123_456]
        {
            // the parser is strict about not accepting separators
            let formatted = format_money("", cents).replace(',', "");
            assert_eq!(parse_amount(&formatted), Ok(cents), "{}", formatted);
        }
    }

    #[test]
    fn the_separators_that_are_shown_are_not_accepted_back() {
        let shown = format_money("", 123_456);

        assert_eq!(shown, "1,234.56");
        assert_eq!(parse_amount(&shown), Err(ParseAmountError::Invalid));
    }

    #[test]
    fn convert_rounds_to_the_nearest_cent() {
        assert_eq!(convert_cents(10_000, 1.0), 10_000);
        assert_eq!(convert_cents(10_000, 1.25), 12_500);
        assert_eq!(convert_cents(10_000, 0.5), 5_000);
        // 3 cents * 0.5 = 1.5 -> rounds away from zero
        assert_eq!(convert_cents(3, 0.5), 2);
        assert_eq!(convert_cents(1, 0.4), 0);
        assert_eq!(convert_cents(1, 0.6), 1);
        assert_eq!(convert_cents(-1_000, 1.1), -1_100);
        assert_eq!(convert_cents(0, 1.23), 0);
    }

    #[test]
    fn blank_input_means_there_are_no_more_amounts() {
        for input in ["", " ", "   ", "\t", " \t "] {
            assert_eq!(parse_optional_amount(input), Ok(None), "{:?}", input);
        }
    }

    #[test]
    fn optional_amounts_are_parsed_like_other_amounts() {
        assert_eq!(parse_optional_amount("12.34"), Ok(Some(1234)));
        assert_eq!(parse_optional_amount("-0.5"), Ok(Some(-50)));
        assert_eq!(parse_optional_amount(" 8.2 "), Ok(Some(820)));
        assert_eq!(
            parse_optional_amount("abc"),
            Err(ParseAmountError::Invalid)
        );
        assert_eq!(
            parse_optional_amount("99999999999999999999"),
            Err(ParseAmountError::OutOfRange)
        );
    }

    #[test]
    fn zero_is_an_amount_not_the_end() {
        assert_eq!(parse_optional_amount("0"), Ok(Some(0)));
        assert_eq!(parse_optional_amount("-0.00"), Ok(Some(0)));
    }

    #[test]
    fn parse_errors_describe_the_problem() {
        assert_eq!(ParseAmountError::Invalid.to_string(), "invalid amount");
        assert_eq!(
            ParseAmountError::OutOfRange.to_string(),
            "amount out of range"
        );

        let boxed: Box<dyn Error> = Box::new(ParseAmountError::Invalid);
        assert_eq!(boxed.to_string(), "invalid amount");
    }
}
