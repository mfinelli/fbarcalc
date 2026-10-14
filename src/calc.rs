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
use inquire::ui::RenderConfig;
use inquire::CustomType;
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
    let currency_code = currency.code;
    let currency_symbol = currency.symbol;

    // don't make the user type in all of their transactions only to find out
    // at the end that we can't convert the result
    if let Err(e) =
        exchange::ensure_configured(conf.fca_api_key.as_deref(), currency_code)
    {
        eprintln!("error: {}", e);
        return ExitCode::FAILURE;
    }

    let format = |cents: i64| money::format_money(currency_symbol, cents);
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

    let start = match amount.prompt() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::FAILURE;
        }
    };

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
            parser: &|i| if i.trim().is_empty() {
                Ok(None)
            } else {
                money::parse_amount(i).map(Some).map_err(|_| ())
            },
            render_config: RenderConfig::default(),
        };

        match p.prompt() {
            Ok(Some(v)) => transactions.push(v),
            Ok(None) => break,
            Err(e) => {
                eprintln!("error: {}", e);
                return ExitCode::FAILURE;
            }
        }
    }

    let max = match max_balance(start, &transactions) {
        Some(m) => m,
        None => {
            eprintln!("error: the account balance is too large to calculate");
            return ExitCode::FAILURE;
        }
    };

    println!();
    println!(
        "Maximum value in {}: {}",
        currency_code,
        money::format_money(currency_symbol, max)
    );

    // the maximum is the important part so show it before we try to convert
    // it, that way it's not lost if the exchange rate lookup fails
    let exchange_rate = match exchange::usd_per_unit(
        conf.fca_api_key.as_deref(),
        currency_code,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    println!(
        "Maximum value in USD: {}",
        money::format_money("$", money::convert_cents(max, exchange_rate))
    );

    ExitCode::SUCCESS
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
}
