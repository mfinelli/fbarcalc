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

extern crate dirs;

use fbarcalc::*;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    let config_file = match cli.config {
        Some(c) => {
            let result = c.try_exists();
            if result.is_err() {
                println!("there was an error checking the config file");
                return ExitCode::FAILURE;
            } else {
                if result.unwrap() {
                    c
                } else {
                    println!("given config file doesn't exist");
                    return ExitCode::FAILURE;
                }
            }
        }
        None => {
            let mut config_path = dirs::config_dir().unwrap();
            config_path.push("fbarcalc");
            config_path.push("config.toml");

            let result = config_path.try_exists();
            if result.is_err() {
                println!("there was an error checking the config file");
                return ExitCode::FAILURE;
            } else {
                let result = result.unwrap();
                if result {
                    config_path
                } else {
                    println!("error: the config file doesn't exist yet!");
                    println!("       run fbarcalc config to create it");
                    return ExitCode::FAILURE;
                }
            }
        }
    };

    match &cli.command {
        Some(cli::Commands::Config{}) => {
            config::configure();
        }
        None => {
            println!("{:?}", calc::calculate(0.0));
        }
    }

    ExitCode::SUCCESS
}
