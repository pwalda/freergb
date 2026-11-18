// SPDX-License-Identifier: GPL-2.0-or-later
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod about;
mod diag;
mod dto;
mod lights;
mod schedule;
mod shell;
mod store;
mod udev;
mod update;

fn main() {
    shell::run();
}
