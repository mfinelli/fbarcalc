/* fbarcalc: find account maximum value
 * Copyright 2024 Mario Finelli
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
use inquire::ui::RenderConfig;
use inquire::CustomType;
use ordered_float::NotNan;
use std::collections::BinaryHeap;

pub fn calculate(conf: config::Config) -> f64 {
    let mut heap = BinaryHeap::new();

    let currency_code = match conf.default_input_currency {
        None => config::select_input_currency(None, false).unwrap().code,
        Some(c) => {
            config::select_input_currency(Some(c.as_str()), false)
                .unwrap()
                .code
        }
    };

    let currency_symbol = config::SUPPORTED_CURRENCIES
        .iter()
        .find(|c| c.code == currency_code)
        .unwrap()
        .symbol;

    let mut start: f64;

    let amount: CustomType<f64> = CustomType {
        message: "Starting value:",
        starting_input: None,
        formatter: &|i| format!("{}{:.2}", currency_symbol, truncate_to_two(i)),
        default_value_formatter: &|i| format!("{}{:.2}", currency_symbol, truncate_to_two(i)),
        default: None,
        validators: vec![],
        placeholder: Some("12.34"),
        error_message: "Please enter a valid amount.".into(),
        help_message: "Do not use the currency symbol and the number should use dots as the decimal separator.".into(),
        parser: &|i| match i.parse::<f64>() {
                Ok(v) => Ok(truncate_to_two(v)),
                Err(_) => Err(()),
        },
        render_config: RenderConfig::default(),
    };

    match amount.prompt() {
        Ok(v) => start = v,
        Err(e) => panic!("{:?}", e),
    };
    heap.push(NotNan::new(start).unwrap());

    loop {
        let p: CustomType<Option<f64>> = CustomType {
            message: "Next Transaction:",
            starting_input: None,
            formatter: &|i| match i {
                Some(i) => format!("{}{:.2}", currency_symbol, truncate_to_two(i)),
                None => String::new(),
            },
            default_value_formatter: &|i| match i {
                Some(i) => format!("{}{:.2}", currency_symbol, truncate_to_two(i)),
                None => String::new(),
            },
            default: None,
            validators: vec![],
            placeholder: Some("12.34"),
            error_message: "Please enter a valid amount or empty to finish.".into(),
            help_message: "Do not use the currency symbol and the number should use dots as the decimal separator. Enter an empty value to finish.".into(),
            parser: &|i| if i.is_empty() {
                Ok(None)
            } else {
                match i.parse::<f64>() {
                    Ok(v) => Ok(Some(truncate_to_two(v))),
                    Err(_) => Err(()),
                }
            },
            render_config: RenderConfig::default(),
        };

        match p.prompt() {
            Ok(v) => match v {
                Some(v) => {
                    start += v;
                    heap.push(NotNan::new(start).unwrap());
                }
                None => break,
            },
            Err(e) => panic!("{:?}", e),
        }
    }

    heap.pop().unwrap().into_inner()
}

// https://stackoverflow.com/a/63214916
fn truncate_to_two(before: f64) -> f64 {
    (before * 100.0).floor() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_to_two() {
        assert_eq!(truncate_to_two(12.345), 12.34);
    }
}
