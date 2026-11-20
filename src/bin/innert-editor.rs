//! The model editor, its own app: every model and animation of the game shown alone in a studio,
//! to change, see src\editor.rs and assets\models\README.md
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    innert::editor_main();
}
