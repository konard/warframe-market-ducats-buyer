use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::lib;
use crate::lib::storage::Storage;

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    max_price_to_search: String,
    min_quantity_to_search: String,
    price_to_offer: String,
    item_names: String,
}

impl Settings {
    // Getters
    pub fn max_price_to_search(&self) -> &str {
        &self.max_price_to_search
    }

    pub fn min_quantity_to_search(&self) -> &str {
        &self.min_quantity_to_search
    }

    pub fn price_to_offer(&self) -> &str {
        &self.price_to_offer
    }

    pub fn item_names(&self) -> &str {
        &self.item_names
    }

    // Setters
    pub fn set_max_price_to_search(&mut self, value: String) {
        self.max_price_to_search = value;
    }

    pub fn set_min_quantity_to_search(&mut self, value: String) {
        self.min_quantity_to_search = value;
    }

    pub fn set_price_to_offer(&mut self, value: String) {
        self.price_to_offer = value;
    }

    pub fn set_item_names(&mut self, value: String) {
        self.item_names = value;
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_price_to_search: lib::MAX_PRICE_TO_SEARCH.to_string(),
            min_quantity_to_search: lib::MIN_QUANTITY_TO_SEARCH.to_string(),
            price_to_offer: lib::PRICE_TO_OFFER.to_string(),
            item_names: lib::PROFITABLE_ITEM_NAMES.join("\n").to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SettingsManager {
    current_settings: Settings,
    pub(crate) presets: HashMap<String, Settings>,
    pub(crate) current_preset_name: Option<String>,

    #[serde(default)]
    pub ignored_user_nicknames: Vec<String>,

    #[serde(default)]
    pub contacted_order_ids: Vec<String>,
}

impl SettingsManager {
    const STORAGE_KEY: &'static str = "app_settings";

    pub fn load() -> Self {
        let storage = Storage::new();
        match storage.get(Self::STORAGE_KEY) {
            Ok(Some(data)) => serde_json::from_str(&data).unwrap_or_default(),
            _ => Self::default(),
        }
    }

    pub fn save(&self) {
        let storage = Storage::new();
        if let Ok(data) = serde_json::to_string(self) {
            let _ = storage.set(Self::STORAGE_KEY, &data);
        }
    }

    pub fn save_as_preset(&mut self, name: String) {
        self.presets.insert(name.clone(), self.current_settings.clone());
        self.current_preset_name = Some(name);
        self.save();
    }

    pub fn load_preset(&mut self, name: &str) -> bool {
        if let Some(settings) = self.presets.get(name) {
            self.current_settings = settings.clone();
            self.current_preset_name = Some(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn delete_preset(&mut self, name: &str) {
        self.presets.remove(name);
        if self.current_preset_name.as_deref() == Some(name) {
            self.current_preset_name = None;
        }
        self.save();
    }

    pub fn reset_settings(&mut self) {
        self.current_settings = Settings::default();
    }

    pub fn delete_all_presets(&mut self) {
        self.presets.clear();
        self.current_preset_name = None;
        self.save();
    }

    pub fn get_current_settings(&self) -> &Settings {
        &self.current_settings
    }

    pub fn get_current_settings_mut(&mut self) -> &mut Settings {
        &mut self.current_settings
    }

    pub fn get_presets(&self) -> &HashMap<String, Settings> {
        &self.presets
    }

    pub fn get_current_preset_name(&self) -> Option<&str> {
        self.current_preset_name.as_deref()
    }

    /// Update the currently loaded preset (if any) with the current settings.
    /// Returns true if updated, false if there was no current preset selected.
    pub fn update_current_preset(&mut self) -> bool {
        if let Some(ref name) = self.current_preset_name {
            if let Some(settings) = self.presets.get_mut(name) {
                *settings = self.current_settings.clone();
                self.save();
                return true;
            }
        }
        false
    }

    pub fn ignored_user_nicknames(&self) -> &Vec<String> {
        &self.ignored_user_nicknames
    }

    pub fn set_ignored_user_nicknames(&mut self, nicknames: Vec<String>) {
        self.ignored_user_nicknames = nicknames;
        self.save();
    }

    pub fn add_ignored_user_nickname(&mut self, nickname: String) {
        if !self.ignored_user_nicknames.contains(&nickname) {
            self.ignored_user_nicknames.push(nickname);
            self.save();
        }
    }

    pub fn remove_ignored_user_nickname(&mut self, nickname: &str) {
        self.ignored_user_nicknames.retain(|n| n != nickname);
        self.save();
    }

    pub fn contacted_order_ids(&self) -> &Vec<String> {
        &self.contacted_order_ids
    }
    pub fn set_contacted_order_ids(&mut self, ids: Vec<String>) {
        self.contacted_order_ids = ids;
        self.save();
    }
    pub fn add_contacted_order_id(&mut self, id: String) {
        if !self.contacted_order_ids.contains(&id) {
            self.contacted_order_ids.push(id);
            self.save();
        }
    }
    pub fn remove_contacted_order_id(&mut self, id: &str) {
        self.contacted_order_ids.retain(|x| x != id);
        self.save();
    }
    pub fn clear_contacted_order_ids(&mut self) {
        self.contacted_order_ids.clear();
        self.save();
    }
}

impl Default for SettingsManager {
    fn default() -> Self {
        Self {
            current_settings: Settings::default(),
            presets: HashMap::new(),
            current_preset_name: None,
            ignored_user_nicknames: Vec::new(),
            contacted_order_ids: Vec::new(),
        }
    }
}