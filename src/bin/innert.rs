// No console window behind the game, except when debugging. Problems are shown in a box, see
// show_error() in src\main.rs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    innert::main();
}
