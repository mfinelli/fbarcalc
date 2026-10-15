/* fbarcalc: find account maximum value
 * Copyright 2024-2026 Mario Finelli
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

use crate::config;
use crate::exchange;
use crate::money;
use inquire::CustomType;
use inquire::error::InquireError;
use inquire::ui::RenderConfig;
use std::error::Error;
use std::io::Write;
use std::process::ExitCode;

/// The balance of the account at the start and after each transaction, or
/// `None` if the running total overflows.
fn running_balances(start: i64, transactions: &[i64]) -> Option<Vec<i64>> {
    let mut balance = start;
    let mut balances = Vec::with_capacity(transactions.len() + 1);
    balances.push(balance);

    for &t in transactions {
        balance = balance.checked_add(t)?;
        balances.push(balance);
    }

    Some(balances)
}

/// Find the highest balance that the account reaches, including the starting
/// balance and the balance after each transaction, or `None` if the running
/// total overflows.
pub fn max_balance(start: i64, transactions: &[i64]) -> Option<i64> {
    running_balances(start, transactions)?.into_iter().max()
}

/// The balance of the account after the last transaction (the starting
/// balance if there aren't any), or `None` if the running total overflows.
pub fn final_balance(start: i64, transactions: &[i64]) -> Option<i64> {
    running_balances(start, transactions)?.last().copied()
}

/// The account activity as a table: the balance at the start and after each
/// transaction, with the balances that are the maximum marked. `None` if the
/// running total overflows.
fn audit_trail(
    symbol: &str,
    start: i64,
    transactions: &[i64],
) -> Option<String> {
    const HEADER: [&str; 3] = ["#", "Transaction", "Balance"];

    let balances = running_balances(start, transactions)?;
    let max = balances.iter().copied().max()?;

    let rows: Vec<[String; 3]> = balances
        .iter()
        .enumerate()
        .map(|(i, &balance)| {
            let transaction = match i.checked_sub(1) {
                None => String::from("start"),
                Some(t) => money::format_money(symbol, transactions[t]),
            };

            [
                i.to_string(),
                transaction,
                money::format_money(symbol, balance),
            ]
        })
        .collect();

    // line up the columns, to the right as they are mostly numbers
    let widths: Vec<usize> = (0..HEADER.len())
        .map(|c| {
            rows.iter()
                .map(|row| row[c].chars().count())
                .chain([HEADER[c].chars().count()])
                .max()
                .unwrap_or_default()
        })
        .collect();
    let line = |cells: [&str; 3]| {
        format!(
            "{:>w0$}  {:>w1$}  {:>w2$}",
            cells[0],
            cells[1],
            cells[2],
            w0 = widths[0],
            w1 = widths[1],
            w2 = widths[2]
        )
    };

    let mut lines = vec![line(HEADER)];
    for (row, &balance) in rows.iter().zip(&balances) {
        let mut l = line([&row[0], &row[1], &row[2]]);
        if balance == max {
            l.push_str("  <- maximum");
        }
        lines.push(l);
    }

    Some(lines.join("\n") + "\n")
}

pub fn calculate(conf: config::Config, verbose: bool) -> ExitCode {
    let currency = match config::select_input_currency(
        conf.default_input_currency.as_deref(),
        false,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // don't make the user type in all of their transactions only to find out
    // at the end that we can't convert the result
    if let Err(e) =
        exchange::ensure_configured(conf.fca_api_key.as_deref(), currency.code)
    {
        eprintln!("error: {}", e);
        return ExitCode::FAILURE;
    }

    let (start, transactions) = match prompt_amounts(currency) {
        Ok(amounts) => amounts,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    report(
        currency,
        start,
        &transactions,
        verbose,
        || exchange::usd_per_unit(conf.fca_api_key.as_deref(), currency.code),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    )
}

/// Ask for the starting value and then each of the transactions.
fn prompt_amounts(
    currency: &config::Currency,
) -> Result<(i64, Vec<i64>), InquireError> {
    let format = |cents: i64| money::format_money(currency.symbol, cents);
    let format_optional =
        |cents: Option<i64>| cents.map(format).unwrap_or_default();

    let amount: CustomType<i64> = CustomType {
        message: "Starting value:",
        starting_input: None,
        formatter: &format,
        default_value_formatter: &format,
        default: None,
        validators: vec![],
        placeholder: Some("12.34"),
        error_message: "Please enter a valid amount.".into(),
        help_message: "Do not use the currency symbol and the number should use dots as the decimal separator.".into(),
        parser: &|i| money::parse_amount(i).map_err(|_| ()),
        render_config: RenderConfig::default(),
    };

    let start = amount.prompt()?;
    let mut transactions = Vec::new();

    loop {
        let p: CustomType<Option<i64>> = CustomType {
            message: "Next Transaction:",
            starting_input: None,
            formatter: &format_optional,
            default_value_formatter: &format_optional,
            default: None,
            validators: vec![],
            placeholder: Some("12.34"),
            error_message: "Please enter a valid amount or empty to finish.".into(),
            help_message: "Do not use the currency symbol and the number should use dots as the decimal separator. Enter an empty value to finish.".into(),
            parser: &|i| money::parse_optional_amount(i).map_err(|_| ()),
            render_config: RenderConfig::default(),
        };

        match p.prompt()? {
            Some(v) => transactions.push(v),
            None => return Ok((start, transactions)),
        }
    }
}

/// Show the result. Anything that goes wrong is reported on `err` and results
/// in a failure exit code.
///
/// The final balance (to check against the statement) and the maximum are
/// shown in the currency of the account, and the maximum in USD. If `verbose`
/// the balance after each transaction and the exchange rate that was used are
/// shown as well. The exchange rate (the value of one unit of the
/// currency in USD) is only looked up once we have something to convert.
pub fn report(
    currency: &config::Currency,
    start: i64,
    transactions: &[i64],
    verbose: bool,
    rate: impl FnOnce() -> Result<f64, exchange::RateError>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> ExitCode {
    match write_report(currency, start, transactions, verbose, rate, out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // if we can't even write the error there's nothing else to do
            let _ = writeln!(err, "error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn write_report(
    currency: &config::Currency,
    start: i64,
    transactions: &[i64],
    verbose: bool,
    rate: impl FnOnce() -> Result<f64, exchange::RateError>,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    const TOO_LARGE: &str = "the account balance is too large to calculate";

    let max = max_balance(start, transactions).ok_or(TOO_LARGE)?;
    let last = final_balance(start, transactions).ok_or(TOO_LARGE)?;

    writeln!(out)?;

    if verbose {
        let trail = audit_trail(currency.symbol, start, transactions)
            .ok_or(TOO_LARGE)?;
        writeln!(out, "{}", trail)?;
    }

    // so that it can be compared with the statement that the transactions
    // came from to make sure that they were all entered correctly
    writeln!(
        out,
        "Final balance in {}: {}",
        currency.code,
        money::format_money(currency.symbol, last)
    )?;
    writeln!(
        out,
        "Maximum value in {}: {}",
        currency.code,
        money::format_money(currency.symbol, max)
    )?;

    // the maximum is the important part so show it before we try to convert
    // it, that way it's not lost if the exchange rate lookup fails
    let rate = rate()?;

    // there's nothing to say about the rate when it's USD to USD
    if verbose && currency.code != "USD" {
        writeln!(out, "Exchange rate: 1 {} = {} USD", currency.code, rate)?;
    }

    writeln!(
        out,
        "Maximum value in USD: {}",
        money::format_money("$", money::convert_cents(max, rate))
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_with_no_transactions_is_the_start() {
        assert_eq!(max_balance(1000, &[]), Some(1000));
        assert_eq!(max_balance(-1000, &[]), Some(-1000));
        assert_eq!(max_balance(0, &[]), Some(0));
    }

    #[test]
    fn max_is_the_start_if_the_balance_only_falls() {
        assert_eq!(max_balance(1000, &[-100, -200, -300]), Some(1000));
    }

    #[test]
    fn max_is_the_end_if_the_balance_only_rises() {
        assert_eq!(max_balance(1000, &[100, 200, 300]), Some(1600));
    }

    #[test]
    fn max_can_be_in_the_middle() {
        // 1000, 1500, 2500, 500, 700
        assert_eq!(max_balance(1000, &[500, 1000, -2000, 200]), Some(2500));
    }

    #[test]
    fn max_is_the_peak_not_the_final_balance() {
        assert_eq!(max_balance(0, &[5000, -5000]), Some(5000));
    }

    #[test]
    fn max_can_be_negative() {
        // -1000, -1500, -1200
        assert_eq!(max_balance(-1000, &[-500, 300]), Some(-1000));
        assert_eq!(max_balance(-2000, &[500, 300]), Some(-1200));
    }

    #[test]
    fn max_can_climb_out_of_the_red() {
        // -1000, -500, 250
        assert_eq!(max_balance(-1000, &[500, 750]), Some(250));
    }

    #[test]
    fn max_handles_repeated_peaks() {
        assert_eq!(max_balance(100, &[100, -100, 100, -100]), Some(200));
    }

    #[test]
    fn max_reports_overflow_instead_of_wrapping() {
        assert_eq!(max_balance(i64::MAX, &[1]), None);
        assert_eq!(max_balance(i64::MIN, &[-1]), None);
        assert_eq!(max_balance(i64::MAX, &[-1, 1]), Some(i64::MAX));
    }

    #[test]
    fn max_with_amounts_parsed_from_input() {
        let amounts: Vec<i64> = ["8.2", "-0.10", "19.99", "-30"]
            .iter()
            .map(|s| money::parse_amount(s).unwrap())
            .collect();

        // 10.00 start, 18.20, 18.10, 38.09, 8.09
        assert_eq!(max_balance(1000, &amounts), Some(3809));
    }

    // report

    fn currency(code: &str) -> &'static config::Currency {
        config::SUPPORTED_CURRENCIES
            .iter()
            .find(|c| c.code == code)
            .unwrap()
    }

    /// Run a report that is able to get the given exchange rate and return
    /// what it wrote to stdout, what it wrote to stderr, and its exit code.
    fn run_report(
        code: &str,
        start: i64,
        transactions: &[i64],
        rate: Result<f64, exchange::RateError>,
    ) -> (String, String, ExitCode) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = report(
            currency(code),
            start,
            transactions,
            false,
            move || rate,
            &mut out,
            &mut err,
        );

        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
            exit,
        )
    }

    /// A writer that can't be written to, like a closed pipe.
    struct ClosedPipe;

    impl Write for ClosedPipe {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "pipe closed",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn rate_must_not_be_requested() -> Result<f64, exchange::RateError> {
        panic!("the exchange rate shouldn't have been requested");
    }

    #[test]
    fn the_report_shows_the_final_balance_and_the_maximum() {
        // 8.20, 8.10, 28.09 and then 23.09: the maximum isn't the final
        // balance. 1 EUR is 2 USD
        let (out, err, exit) =
            run_report("EUR", 820, &[-10, 1999, -500], Ok(2.0));

        assert_eq!(
            out,
            "\nFinal balance in EUR: €23.09\n\
             Maximum value in EUR: €28.09\n\
             Maximum value in USD: $56.18\n"
        );
        assert_eq!(err, "");
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn the_report_for_usd_shows_the_same_value_twice() {
        let (out, err, exit) = run_report("USD", 1000, &[500, -250], Ok(1.0));

        assert_eq!(
            out,
            "\nFinal balance in USD: $12.50\n\
             Maximum value in USD: $15.00\n\
             Maximum value in USD: $15.00\n"
        );
        assert_eq!(err, "");
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn the_report_converts_to_the_nearest_cent() {
        // 3 pence is 4.5 cents
        let (out, _, _) = run_report("GBP", 3, &[], Ok(1.5));
        assert!(out.ends_with("Maximum value in USD: $0.05\n"), "{}", out);
    }

    #[test]
    fn the_report_handles_a_negative_maximum() {
        let (out, err, exit) = run_report("JPY", -500, &[-100], Ok(0.5));

        assert_eq!(
            out,
            "\nFinal balance in JPY: -¥6.00\n\
             Maximum value in JPY: -¥5.00\n\
             Maximum value in USD: -$2.50\n"
        );
        assert_eq!(err, "");
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn the_balances_are_still_shown_if_the_rate_cant_be_found() {
        let (out, err, exit) = run_report(
            "EUR",
            820,
            &[-10, 1999, -500],
            Err(exchange::RateError::MissingApiKey),
        );

        assert_eq!(
            out,
            "\nFinal balance in EUR: €23.09\nMaximum value in EUR: €28.09\n"
        );
        assert_eq!(
            err,
            "error: no API key is configured, run `fbarcalc config` to set \
             one\n"
        );
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn the_rate_is_requested_exactly_once() {
        let calls = std::cell::Cell::new(0);
        let (mut out, mut err) = (Vec::new(), Vec::new());

        let exit = report(
            currency("EUR"),
            100,
            &[100, -50],
            false,
            || {
                calls.set(calls.get() + 1);
                Ok(1.0)
            },
            &mut out,
            &mut err,
        );

        assert_eq!(calls.get(), 1);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn an_overflowing_balance_is_reported_before_anything_else() {
        let (mut out, mut err) = (Vec::new(), Vec::new());

        let exit = report(
            currency("EUR"),
            i64::MAX,
            &[1],
            false,
            rate_must_not_be_requested,
            &mut out,
            &mut err,
        );

        assert_eq!(out, b"");
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "error: the account balance is too large to calculate\n"
        );
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn a_closed_stdout_is_an_error_instead_of_a_panic() {
        let mut err = Vec::new();

        let exit = report(
            currency("EUR"),
            100,
            &[],
            false,
            rate_must_not_be_requested,
            &mut ClosedPipe,
            &mut err,
        );

        assert_eq!(String::from_utf8(err).unwrap(), "error: pipe closed\n");
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn a_closed_stderr_is_not_a_panic_either() {
        let exit = report(
            currency("EUR"),
            i64::MAX,
            &[1],
            false,
            rate_must_not_be_requested,
            &mut Vec::new(),
            &mut ClosedPipe,
        );

        assert_eq!(exit, ExitCode::FAILURE);
    }

    // running balances

    #[test]
    fn the_running_balances_start_with_the_starting_balance() {
        assert_eq!(running_balances(500, &[]), Some(vec![500]));
        assert_eq!(
            running_balances(500, &[100, -300, 50]),
            Some(vec![500, 600, 300, 350])
        );
        assert_eq!(running_balances(-5, &[10]), Some(vec![-5, 5]));
    }

    #[test]
    fn the_running_balances_report_overflow() {
        assert_eq!(running_balances(i64::MAX, &[1]), None);
        assert_eq!(running_balances(0, &[i64::MIN, -1]), None);
    }

    // audit trail

    #[test]
    fn the_audit_trail_shows_each_balance_and_marks_the_maximum() {
        let expected = [
            "#  Transaction  Balance",
            "0        start    €8.20",
            "1       -€0.10    €8.10",
            "2       €19.99   €28.09  <- maximum",
        ]
        .join("\n")
            + "\n";

        assert_eq!(audit_trail("€", 820, &[-10, 1999]), Some(expected));
    }

    #[test]
    fn the_audit_trail_marks_every_balance_that_is_the_maximum() {
        let expected = [
            "#  Transaction  Balance",
            "0        start    $1.00",
            "1        $1.00    $2.00  <- maximum",
            "2       -$1.00    $1.00",
            "3        $1.00    $2.00  <- maximum",
        ]
        .join("\n")
            + "\n";

        assert_eq!(audit_trail("$", 100, &[100, -100, 100]), Some(expected));
    }

    #[test]
    fn the_audit_trail_with_no_transactions_is_just_the_start() {
        let expected = [
            "#  Transaction  Balance",
            "0        start    $5.00  <- maximum",
        ]
        .join("\n")
            + "\n";

        assert_eq!(audit_trail("$", 500, &[]), Some(expected));
    }

    #[test]
    fn the_audit_trail_handles_negative_balances() {
        let expected = [
            "#  Transaction  Balance",
            "0        start   -£5.00",
            "1       -£1.00   -£6.00",
            "2        £3.00   -£3.00  <- maximum",
        ]
        .join("\n")
            + "\n";

        assert_eq!(audit_trail("£", -500, &[-100, 300]), Some(expected));
    }

    #[test]
    fn the_audit_trail_widens_the_number_column_for_many_transactions() {
        let transactions = [100; 11];
        let trail = audit_trail("$", 0, &transactions).unwrap();
        let lines: Vec<&str> = trail.lines().collect();

        assert_eq!(lines.len(), 13);
        assert_eq!(lines[0], " #  Transaction  Balance");
        assert!(lines[1].starts_with(" 0  "), "{}", lines[1]);
        assert!(lines[10].starts_with(" 9  "), "{}", lines[10]);
        assert!(lines[12].starts_with("11  "), "{}", lines[12]);
        assert!(lines[12].ends_with("$11.00  <- maximum"), "{}", lines[12]);
    }

    #[test]
    fn the_audit_trail_lines_up_whatever_the_size_of_the_amounts() {
        let trail =
            audit_trail("¥", 123_456_789, &[-5, 100_000_000_000, -1]).unwrap();

        // everything but the marker is the same width: one char per
        // character even for the multi-byte symbol
        let widths: Vec<usize> = trail
            .lines()
            .map(|l| l.trim_end_matches("  <- maximum").chars().count())
            .collect();
        assert!(widths.windows(2).all(|w| w[0] == w[1]), "{:?}", widths);
        assert!(trail.contains("¥1,000,000,000.00"), "{}", trail);
        assert!(trail.contains("¥1,001,234,567.84  <- maximum"), "{}", trail);
    }

    #[test]
    fn the_audit_trail_has_no_trailing_whitespace() {
        let trail = audit_trail("$", 100, &[50, -25]).unwrap();

        assert!(trail.ends_with('\n') && !trail.ends_with("\n\n"));
        for line in trail.lines() {
            assert_eq!(line, line.trim_end());
        }
    }

    #[test]
    fn the_audit_trail_reports_overflow() {
        assert_eq!(audit_trail("$", i64::MAX, &[1]), None);
    }

    // verbose report

    fn run_verbose_report(
        code: &str,
        start: i64,
        transactions: &[i64],
        rate: Result<f64, exchange::RateError>,
    ) -> (String, String, ExitCode) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = report(
            currency(code),
            start,
            transactions,
            true,
            move || rate,
            &mut out,
            &mut err,
        );

        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
            exit,
        )
    }

    #[test]
    fn the_verbose_report_shows_the_trail_and_the_rate_used() {
        // 28.09 EUR is 35.11 USD at 1 EUR = 1.25 USD
        let (out, err, exit) =
            run_verbose_report("EUR", 820, &[-10, 1999, -500], Ok(1.25));

        let expected = [
            "",
            "#  Transaction  Balance",
            "0        start    €8.20",
            "1       -€0.10    €8.10",
            "2       €19.99   €28.09  <- maximum",
            "3       -€5.00   €23.09",
            "",
            "Final balance in EUR: €23.09",
            "Maximum value in EUR: €28.09",
            "Exchange rate: 1 EUR = 1.25 USD",
            "Maximum value in USD: $35.11",
        ]
        .join("\n")
            + "\n";

        assert_eq!(out, expected);
        assert_eq!(err, "");
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn the_verbose_report_for_usd_has_no_exchange_rate() {
        let (out, err, exit) = run_verbose_report("USD", 1000, &[500], Ok(1.0));

        let expected = [
            "",
            "#  Transaction  Balance",
            "0        start   $10.00",
            "1        $5.00   $15.00  <- maximum",
            "",
            "Final balance in USD: $15.00",
            "Maximum value in USD: $15.00",
            "Maximum value in USD: $15.00",
        ]
        .join("\n")
            + "\n";

        assert_eq!(out, expected);
        assert_eq!(err, "");
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn the_verbose_report_shows_the_exact_rate_that_was_used() {
        // the value that the conversion is done with and not a rounded one
        let (out, _, _) =
            run_verbose_report("GBP", 100, &[], Ok(1.0869565217391304));

        assert!(
            out.contains("Exchange rate: 1 GBP = 1.0869565217391304 USD\n"),
            "{}",
            out
        );
    }

    #[test]
    fn the_verbose_report_keeps_the_trail_if_the_rate_cant_be_found() {
        let (out, err, exit) = run_verbose_report(
            "EUR",
            820,
            &[-10, 1999, -500],
            Err(exchange::RateError::MissingApiKey),
        );

        let expected = [
            "",
            "#  Transaction  Balance",
            "0        start    €8.20",
            "1       -€0.10    €8.10",
            "2       €19.99   €28.09  <- maximum",
            "3       -€5.00   €23.09",
            "",
            "Final balance in EUR: €23.09",
            "Maximum value in EUR: €28.09",
        ]
        .join("\n")
            + "\n";

        assert_eq!(out, expected);
        assert!(err.starts_with("error: no API key"), "{}", err);
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn the_verbose_report_reports_overflow_before_showing_anything() {
        let (mut out, mut err) = (Vec::new(), Vec::new());

        let exit = report(
            currency("EUR"),
            i64::MAX,
            &[1],
            true,
            rate_must_not_be_requested,
            &mut out,
            &mut err,
        );

        assert_eq!(out, b"");
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "error: the account balance is too large to calculate\n"
        );
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn verbose_only_adds_to_the_normal_report() {
        for (code, start, transactions, rate) in [
            ("EUR", 820, &[-10, 1999][..], 1.25),
            ("USD", 1000, &[500][..], 1.0),
            ("JPY", -500, &[-100][..], 0.5),
            ("GBP", 0, &[][..], 1.3),
        ] {
            let (plain, _, _) = run_report(code, start, transactions, Ok(rate));
            let (verbose, _, _) =
                run_verbose_report(code, start, transactions, Ok(rate));

            let summary = |s: &str| -> Vec<String> {
                s.lines()
                    .filter(|l| {
                        l.starts_with("Final balance")
                            || l.starts_with("Maximum value")
                    })
                    .map(String::from)
                    .collect()
            };
            assert_eq!(summary(&plain), summary(&verbose), "{}", code);
            assert!(verbose.len() > plain.len(), "{}", code);
        }
    }

    // final balance

    #[test]
    fn the_final_balance_is_the_balance_after_the_last_transaction() {
        assert_eq!(final_balance(500, &[100, -300, 50]), Some(350));
        assert_eq!(final_balance(-5, &[10]), Some(5));
        assert_eq!(final_balance(100, &[500, -600]), Some(0));
        assert_eq!(final_balance(100, &[-500]), Some(-400));
    }

    #[test]
    fn the_final_balance_without_transactions_is_the_start() {
        assert_eq!(final_balance(500, &[]), Some(500));
        assert_eq!(final_balance(-500, &[]), Some(-500));
    }

    #[test]
    fn the_final_balance_is_not_the_maximum() {
        assert_eq!(final_balance(0, &[5000, -5000]), Some(0));
        assert_eq!(max_balance(0, &[5000, -5000]), Some(5000));
    }

    #[test]
    fn the_final_balance_reports_overflow() {
        assert_eq!(final_balance(i64::MAX, &[1]), None);
        assert_eq!(final_balance(i64::MAX, &[-1, 1]), Some(i64::MAX));
    }

    #[test]
    fn the_final_balance_is_the_last_row_of_the_audit_trail() {
        let trail = audit_trail("$", 1000, &[250, -1750, 99]).unwrap();
        let last_row = trail.lines().last().unwrap();

        assert_eq!(final_balance(1000, &[250, -1750, 99]), Some(-401));
        assert!(last_row.ends_with("-$4.01"), "{}", last_row);
    }

    #[test]
    fn the_report_shows_the_final_balance_before_the_maximum() {
        let (out, _, _) = run_report("GBP", 500, &[], Ok(2.0));

        // with nothing to add up it's the starting balance
        assert_eq!(
            out,
            "\nFinal balance in GBP: £5.00\n\
             Maximum value in GBP: £5.00\n\
             Maximum value in USD: $10.00\n"
        );
    }

    #[test]
    fn the_report_shows_thousands_separators() {
        // 1,234,567.89 and then 1,235,567.89 and then -764,432.11
        let (out, err, exit) =
            run_report("EUR", 123_456_789, &[100_000, -200_000_000], Ok(1.0));

        assert_eq!(
            out,
            "\nFinal balance in EUR: -€764,432.11\n\
             Maximum value in EUR: €1,235,567.89\n\
             Maximum value in USD: $1,235,567.89\n"
        );
        assert_eq!(err, "");
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn the_audit_trail_shows_thousands_separators() {
        let expected = [
            "#  Transaction    Balance",
            "0        start  €1,500.00",
            "1      €320.75  €1,820.75",
            "2   -€1,200.00    €620.75",
            "3       €80.10    €700.85",
            "4    €2,000.00  €2,700.85  <- maximum",
            "5     -€450.00  €2,250.85",
        ]
        .join("\n")
            + "\n";

        assert_eq!(
            audit_trail(
                "€",
                150_000,
                &[32_075, -120_000, 8_010, 200_000, -45_000]
            ),
            Some(expected)
        );
    }
}
