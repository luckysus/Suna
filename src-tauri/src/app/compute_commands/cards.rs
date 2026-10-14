use super::logic;
use crate::db::DbState;
use serde_json::Value;

#[tauri::command]
pub fn get_steel_model_scientific_cards(
    db: tauri::State<DbState>,
    id: String,
) -> Result<Value, String> {
    logic::get_steel_model_scientific_cards(db, id)
}
