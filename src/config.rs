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

use inquire::Select;
use std::error::Error;

#[derive(Clone, Debug)]
pub struct Currency<'a> {
    code: &'a str,
    name: &'a str,
    symbol: &'a str,
}

#[derive(Clone, Debug)]
pub struct Config<'a> {
    pub version: u8,
    pub default_input_currency: &'a str,
}

pub fn supported_currencies() -> Vec<Currency<'static>> {
    vec![
        Currency {
            code: "EUR",
            name: "Euro",
            symbol: "€",
        },
        Currency {
            code: "GBP",
            name: "British Pound Sterling",
            symbol: "£",
        },
        Currency {
            code: "JPY",
            name: "Japanese Yen",
            symbol: "¥",
        },
        Currency {
            code: "USD",
            name: "US Dollar",
            symbol: "$",
        },
    ]
}

pub fn select_input_currency(default: Option<&str>) -> Result<Currency, Box<dyn Error>> {
    let options = supported_currencies().iter().map(|c| c.name).collect::<Vec<_>>();
    let start = match default {
        // TODO: handle one that we don't have configured
        Some(code) => supported_currencies().iter().position(|c| c.code == code).unwrap(),
        None => 0,
    };

    let ans = Select::new("What is the default input currency?",options).with_starting_cursor(start).prompt();
    match ans {
        Ok(choice) => Ok(supported_currencies().iter().find(|c| c.name == choice).unwrap().clone()),
        Err(e) => panic!("{}", e), // TODO: do better
    }
}

pub fn configure() {
    // let options = supported_currencies().iter().map(|c| c.name).collect::<Vec<_>>();
    // let start = supported_currencies().iter().position(|c| c.name == "Japanese Yen").unwrap();
    // let default_input_currency = Select::new("What is the default input currency?", options).with_starting_cursor(start).prompt();
    // match default_input_currency {
    //     Ok(choice) => println!("{}", choice),
    //     Err(e) => panic!("{}", e),
    // }
    let c = select_input_currency(None).unwrap();
    println!("{:?}", c);
}
