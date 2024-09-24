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

use std::collections::BinaryHeap;
use std::error::Error;
use inquire::{CustomType, InquireError};
use inquire::ui::RenderConfig;
use ordered_float::NotNan;
use crate::config;

pub fn calculate(conf: config::Config) -> f64 {
    // let def_currency = match conf.default_input_currency {
    //     None => None,
    //     Some(c) => {
    //         let a = c.clone();
    //         Some(a.as_str())
    //     },
    // };

    // let currency = config::select_input_currency(def_currency, false).unwrap();

    let mut start = 0.0;
    // let amount: Result<f64, InquireError> = CustomType::new("T:").with_formatter(&|i: f64| format!("{}{:.2}", "$", i)).prompt();
    // println!("{:?}", amount);
    let mut heap = BinaryHeap::new();
    heap.push(NotNan::new(start).unwrap());

    loop {
        let p: CustomType<Option<f64>> = CustomType {
            message: "Next Transaction:",
            starting_input: None,
            formatter: &|i| match i {
                Some(i) => format!("{}{:.2}", "$", truncate_to_two(i)),
                None => format!(""),
            },
            default_value_formatter: &|i| match i {
                Some(i) => format!("{}{:.2}", "$", truncate_to_two(i)),
                None => format!(""),
            },
            default: None,
            validators: vec![],
            placeholder: Some("12.34"),
            error_message: "Please enter a valid amount or empty to finish.".into(),
            help_message: "Do not use the currency symbol and the number should use dots as the decimal separator. Enter an empty value to finish.".into(),
            parser: &|i| if i == "" {
                Ok(None)
            } else {
                match i.parse::<f64>() {
                    Ok(v) => Ok(Some(truncate_to_two(v))),
                    Err(_) => Err(()),
                }
            },
            render_config: RenderConfig::default(),
        };

        // let amount = p.prompt();
        match p.prompt() {
            Ok(v) => match v {
                Some(v) => {
                    start += v;
                    heap.push(NotNan::new(start).unwrap());
                },
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
