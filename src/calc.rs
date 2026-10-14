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
use inquire::error::InquireError;
use inquire::ui::RenderConfig;
use inquire::CustomType;
use std::error::Error;
use std::io::Write;
use std::process::ExitCode;

/// Find the highest balance that the account reaches, including the starting
/// balance and the balance after each transaction, or `None` if the running
/// total overflows.
pub fn max_balance(start: i64, transactions: &[i64]) -> Option<i64> {
    let mut balance = start;
    let mut max = start;

    for &t in transactions {
        balance = balance.checked_add(t)?;
        max = max.max(balance);
    }

    Some(max)
}

pub fn calculate(conf: config::Config) -> ExitCode {
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
/// The exchange rate (the value of one unit of the currency in USD) is only
/// looked up once we have something to convert.
pub fn report(
    currency: &config::Currency,
    start: i64,
    transactions: &[i64],
    rate: impl FnOnce() -> Result<f64, exchange::RateError>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> ExitCode {
    match write_report(currency, start, transactions, rate, out) {
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
    rate: impl FnOnce() -> Result<f64, exchange::RateError>,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let max = max_balance(start, transactions)
        .ok_or("the account balance is too large to calculate")?;

    writeln!(out)?;
    writeln!(
        out,
        "Maximum value in {}: {}",
        currency.code,
        money::format_money(currency.symbol, max)
    )?;

    // the maximum is the important part so show it before we try to convert
    // it, that way it's not lost if the exchange rate lookup fails
    let rate = rate()?;

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
    fn the_report_shows_the_maximum_in_the_currency_and_in_usd() {
        // 8.20, 8.10, 28.09 and 1 EUR is 2 USD
        let (out, err, exit) = run_report("EUR", 820, &[-10, 1999], Ok(2.0));

        assert_eq!(
            out,
            "\nMaximum value in EUR: €28.09\nMaximum value in USD: $56.18\n"
        );
        assert_eq!(err, "");
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn the_report_for_usd_shows_the_same_value_twice() {
        let (out, err, exit) = run_report("USD", 1000, &[500], Ok(1.0));

        assert_eq!(
            out,
            "\nMaximum value in USD: $15.00\nMaximum value in USD: $15.00\n"
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
            "\nMaximum value in JPY: -¥5.00\nMaximum value in USD: -$2.50\n"
        );
        assert_eq!(err, "");
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn the_maximum_is_still_shown_if_the_rate_cant_be_found() {
        let (out, err, exit) = run_report(
            "EUR",
            820,
            &[-10, 1999],
            Err(exchange::RateError::MissingApiKey),
        );

        assert_eq!(out, "\nMaximum value in EUR: €28.09\n");
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
            rate_must_not_be_requested,
            &mut Vec::new(),
            &mut ClosedPipe,
        );

        assert_eq!(exit, ExitCode::FAILURE);
    }
}
