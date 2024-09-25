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

use clap::Parser;
use fbarcalc::*;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    match &cli.command {
        Some(cli::Commands::Config {}) => config::configure(cli.config),
        None => {
            // TODO: make this return a result and handle an error with
            // an ExitCode
            let c = config::get_config(cli.config);
            calc::calculate(c)
        }
    }
}
