#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Hide console on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // It's an example

mod external_lib;
mod lib;

use std::collections::HashSet;
use eframe::egui;
use eframe::egui::{
  Align, Button, DragValue, Frame, Layout, Rounding, ScrollArea, Spinner,
  Stroke, TextEdit,
};
use egui_notify::Toasts;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, TryRecvError};
use std::fs::{OpenOptions};
use std::io::{Write, BufReader, BufRead};
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub struct Preset {
  name: String,
  max_price_to_search: String,
  min_quantity_to_search: String,
  price_to_offer: String,
  item_names: String,
}

struct MyApp {
  rx_fetch: mpsc::Receiver<Result<Vec<lib::Order>, String>>,
  tx_fetch: mpsc::Sender<Result<Vec<lib::Order>, String>>,
  rx_process: mpsc::Receiver<Result<Vec<lib::Order>, String>>,
  tx_process: mpsc::Sender<Result<Vec<lib::Order>, String>>,
  orders: Option<Vec<lib::Order>>,
  processed_orders: Option<Vec<lib::Order>>,
  loading_fetch: bool,
  loading_process: bool,
  settings_manager: lib::settings::SettingsManager,
  toasts: Toasts,
  show_settings: bool,
  show_credits: bool,
  show_all_orders: bool,
  new_preset_name: String,
  show_delete_presets_confirmation: bool,
}

impl Default for MyApp {
  fn default() -> Self {
    let (tx_fetch, rx_fetch) = mpsc::channel();
    let (tx_process, rx_process) = mpsc::channel();
    Self {
      rx_fetch,
      tx_fetch,
      rx_process,
      tx_process,
      orders: None,
      processed_orders: None,
      loading_fetch: false,
      loading_process: false,
      settings_manager: lib::settings::SettingsManager::load(),
      toasts: Toasts::new(),
      show_settings: false,
      show_credits: false,
      show_all_orders: false,
      new_preset_name: String::new(),
      show_delete_presets_confirmation: false,
    }
  }
}

impl eframe::App for MyApp {
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // Poll the fetch channel for new messages
    match self.rx_fetch.try_recv() {
      Ok(result) => {
        match result {
          Ok(data) => {
            let message = format!("Successfully received fetched {:?} orders", data.len());
            info!("{}", message);
            self.orders = Some(data);
            self.toasts.success(message);
          }
          Err(err) => {
            error!("Error fetching orders: {}", err);
            self.toasts.error(format!("Error fetching orders: {}", err));
            self.orders = None;
          }
        }
        self.loading_fetch = false;
      }
      Err(TryRecvError::Empty) => {}
      Err(TryRecvError::Disconnected) => {
        warn!("Fetch channel disconnected.");
        self.loading_fetch = false;
        self.toasts.warning("Fetch channel disconnected.");
      }
    }

    // Poll the process channel for new messages
    match self.rx_process.try_recv() {
      Ok(result) => {
        match result {
          Ok(data) => {
            self.processed_orders = Some(data);
            self.toasts.success("Successfully processed orders.");
          }
          Err(err) => {
            self.toasts.error(format!("Error processing orders: {}", err));
            self.processed_orders = None;
          }
        }
        self.loading_process = false;
      }
      Err(TryRecvError::Empty) => {}
      Err(TryRecvError::Disconnected) => {
        self.loading_process = false;
        self.toasts.warning("Process channel disconnected.");
      }
    }

    let settings = self.settings_manager.get_current_settings();
    let max_price = settings.max_price_to_search().parse::<u32>().unwrap_or_default();
    let min_quantity = settings.min_quantity_to_search().parse::<u32>().unwrap_or_default();
    let offer_price = settings.price_to_offer().parse::<u32>().unwrap_or_default();
    let item_names: Vec<String> = settings
        .item_names()
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    egui::CentralPanel::default().show(ctx, |ui| {
      ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
        ui.vertical_centered(|ui| {
          #[cfg(feature = "debug_ui")]
          {
            if ui.button("Test Toast Notifications").clicked() {
              self.toasts.success("Successfully fetched orders.");
            }
          }

          ui.heading("Warframe Market Ducats Buyer");
          ui.add_space(20.0);

          if ui.button("Credits").clicked() {
            self.show_credits = !self.show_credits;
          }

          ui.add_space(20.0);

          if ui.button("Settings").clicked() {
            self.show_settings = !self.show_settings;
          }

          ui.add_space(20.0);

          let is_enabled_button_fetch_orders = !self.loading_fetch;
          ui.add_enabled_ui(is_enabled_button_fetch_orders, |ui| {
            if ui
                .add_sized([150.0, 30.0], Button::new("Fetch Orders"))
                .clicked()
            {
              info!("Starting to fetch orders...");
              self.loading_fetch = true;
              let tx = self.tx_fetch.clone();
              let item_names = item_names.clone();

              std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(async {
                  match lib::fetch_all_orders(&item_names).await {
                    Ok(orders) => {
                      info!("Successfully fetched orders.");
                      Ok(orders)
                    }
                    Err(e) => {
                      error!("Failed to fetch orders: {:?}", e);
                      Err(format!("{:?}", e))
                    }
                  }
                });
                let _ = tx.send(result);
              });
            }
          });

          let orders_len = self.orders.as_ref().map_or(0, |orders| orders.len());
          ui.label(format!("Orders length: {}", orders_len));

          if ui.button("Show All Orders").clicked() {
            self.show_all_orders = !self.show_all_orders;
          }

          let is_enabled_button_process_orders = !self.loading_process && orders_len > 0;
          ui.add_enabled_ui(is_enabled_button_process_orders, |ui| {
            if ui
                .add_sized([150.0, 30.0], Button::new("Filter & Process Orders"))
                .clicked()
            {
              self.loading_process = true;
              let tx = self.tx_process.clone();
              let orders = self.orders.clone();
              let max_price = max_price;
              let min_quantity = min_quantity;

              let contacted_order_ids: std::collections::HashSet<_> =
                  self.settings_manager.contacted_order_ids().iter().cloned().collect();
              let ignored_nicknames = self.settings_manager.ignored_user_nicknames().iter().cloned().collect::<std::collections::HashSet<_>>();

              let offer_price = offer_price;
              std::thread::spawn(move || {
                let filter_orders = |order: &lib::Order| -> bool {
                  order.user.status == "ingame"
                      && order.visible
                      && order.order_type == "sell"
                      && order.platinum <= max_price
                      && order.quantity >= min_quantity
                      && !contacted_order_ids.contains(&order.id)
                      && !ignored_nicknames.contains(&order.user.ingame_name)
                };

                let processed_orders = orders
                    .map(|o| lib::process_orders(o, filter_orders, offer_price))
                    .unwrap_or_else(Vec::new);
                let _ = tx.send(Ok(processed_orders));
              });
            }
          });
        });

        let processed_orders_len = self.processed_orders.as_ref().map_or(0, |orders| orders.len());
        ui.label(format!("Processed orders length: {}", processed_orders_len));

        if self.loading_fetch {
          ui.add(Spinner::new().size(32.0));
        }

        if let Some(processed_orders) = &self.processed_orders {
          ui.label("Processed Orders:");
          ui.add_space(10.0);

          ScrollArea::new(true).show(ui, |ui| {
            for (i, order) in processed_orders.iter().enumerate() {
              let frame_stroke = if order.is_with_group.unwrap_or(false) {
                Stroke::new(2.0, ui.visuals().selection.stroke.color)
              } else {
                Stroke::new(1.0, ui.visuals().extreme_bg_color)
              };

              let message = lib::generate_message_from_order(order);

              Frame::none()
                  .stroke(frame_stroke)
                  .rounding(Rounding::same(5))
                  .show(ui, |ui| {
                    let button = ui.add_sized([100.0, 100.0], Button::new(message.clone()));
                    if button.clicked() {
                      ui.ctx().copy_text(message.clone());
                      self.settings_manager.add_contacted_order_id(order.id.clone());
                    }
                  });

              ui.add_space(8.0);
            }
          });
        }
      });
    });

    if self.show_settings {
      egui::Window::new("Settings")
          .open(&mut self.show_settings)
          .resizable(true)
          .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                  ui.horizontal(|ui| {
                    if ui.button("Reset Settings to Defaults").clicked() {
                      self.settings_manager.reset_settings();
                    }

                    if ui.button("Save Settings").clicked() {
                      self.settings_manager.save();
                    }
                  });

                  ui.add_space(10.0);

                  // --- Preset settings section ---
                  ui.group(|ui| {
                    ui.heading("Preset Settings");

                    // Presets section
                    ui.group(|ui| {
                      ui.label(egui::RichText::new("Presets").strong());

                      let preset_data: Vec<(String, bool)> = self.settings_manager
                          .get_presets()
                          .iter()
                          .map(|preset| {
                            let is_current = self.settings_manager
                                .get_current_preset_name()
                                .map_or(false, |c| c == preset.name);
                            (preset.name.clone(), is_current)
                          })
                          .collect();

                      if preset_data.is_empty() {
                        ui.label("No presets saved.");
                      } else {
                        for (preset_name, is_current) in &preset_data {
                          egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.vertical(|ui| {
                              // Highlight current
                              if *is_current {
                                ui.colored_label(egui::Color32::LIGHT_GREEN, preset_name);
                              } else {
                                ui.label(preset_name);
                              }
                              ui.horizontal(|ui| {
                                if ui.button("Load").on_hover_text("Load this preset").clicked() {
                                  self.settings_manager.load_preset(preset_name);
                                }
                                if ui.button("Delete").on_hover_text("Delete this preset").clicked() {
                                  self.settings_manager.delete_preset(preset_name);
                                }
                                if *is_current {
                                  if ui.button("Update").on_hover_text("Update this preset with current settings").clicked() {
                                    if self.settings_manager.update_current_preset() {
                                      self.toasts.success("Preset updated with current settings");
                                    } else {
                                      self.toasts.error("Failed to update preset");
                                    }
                                  }
                                }
                              });
                            });
                          });
                          ui.add_space(4.0);
                        }
                      }

                      if !preset_data.is_empty() {
                        if ui.button("Delete All Presets")
                            .on_hover_text("Warning: This will permanently delete all saved presets")
                            .clicked()
                        {
                          self.show_delete_presets_confirmation = true;
                        }
                        ui.add_space(8.0);
                      }

                      // "Save as Preset" section
                      ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.new_preset_name)
                            .on_hover_text("Enter preset name");
                        if ui.button("Save as Preset")
                            .on_hover_text("Save current settings as a new preset")
                            .clicked() && !self.new_preset_name.is_empty()
                        {
                          self.settings_manager.save_as_preset(self.new_preset_name.clone());
                          self.new_preset_name.clear();
                        }
                      });
                    });

                    ui.add_space(10.0);

                    // Settings fields (per-preset)
                    let settings = self.settings_manager.get_current_settings_mut();

                    ui.label("Max Price:");
                    if let Ok(mut value) = settings.max_price_to_search().parse::<u32>() {
                      if ui.add(DragValue::new(&mut value).clamp_range(0..=10).speed(0.02)).changed() {
                        settings.set_max_price_to_search(value.to_string());
                      }
                    }

                    ui.label("Min Quantity:");
                    if let Ok(mut value) = settings.min_quantity_to_search().parse::<u32>() {
                      if ui.add(DragValue::new(&mut value).clamp_range(0..=10).speed(0.02)).changed() {
                        settings.set_min_quantity_to_search(value.to_string());
                      }
                    }

                    ui.label("Offer Price:");
                    if let Ok(mut value) = settings.price_to_offer().parse::<u32>() {
                      if ui.add(DragValue::new(&mut value).clamp_range(0..=10).speed(0.02)).changed() {
                        settings.set_price_to_offer(value.to_string());
                      }
                    }

                    ui.add_space(10.0);

                    let mut item_names = settings.item_names().to_string();
                    ui.label("Item Names (one per line):");
                    if ui.add(
                      TextEdit::multiline(&mut item_names)
                          .hint_text("Enter item names (one per line)")
                          .desired_width(f32::INFINITY)
                          .min_size([ui.available_width(), 100.0].into()),
                    ).changed() {
                      settings.set_item_names(item_names);
                    }
                  });

                  ui.add_space(16.0);

                  // --- Global settings section ---
                  ui.group(|ui| {
                    ui.heading("Global Settings");
                    ui.label("Ignored User Nicknames (one per line):");
                    let mut ignored_nicknames_str = self.settings_manager.ignored_user_nicknames().join("\n");
                    if ui.add(
                      TextEdit::multiline(&mut ignored_nicknames_str)
                          .hint_text("Enter user nicknames to ignore (one per line)")
                          .desired_width(f32::INFINITY)
                          .min_size([ui.available_width(), 60.0].into()),
                    ).changed() {
                      let new_list: Vec<String> = ignored_nicknames_str
                          .lines()
                          .map(|s| s.trim().to_string())
                          .filter(|s| !s.is_empty())
                          .collect();
                      self.settings_manager.set_ignored_user_nicknames(new_list);
                    }

                    ui.add_space(10.0);
                    ui.label("Contacted Order IDs (one per line):");
                    let mut contacted_ids = self.settings_manager.contacted_order_ids().join("\n");
                    if ui.add(
                      TextEdit::multiline(&mut contacted_ids)
                          .hint_text("Order IDs you have already contacted (one per line)")
                          .desired_width(f32::INFINITY)
                          .min_size([ui.available_width(), 60.0].into()),
                    ).changed() {
                      let new_list: Vec<String> = contacted_ids
                          .lines()
                          .map(|s| s.trim().to_string())
                          .filter(|s| !s.is_empty())
                          .collect();
                      self.settings_manager.set_contacted_order_ids(new_list);
                    }
                    if ui.button("Clear All Contacted Order IDs")
                        .on_hover_text("Remove all contacted order IDs")
                        .clicked() {
                      self.settings_manager.clear_contacted_order_ids();
                      self.toasts.success("All contacted order IDs cleared");
                    }
                  });

                });
          });
    }

    if self.show_credits {
      egui::Window::new("Credits")
          .open(&mut self.show_credits)
          .resizable(true)
          .show(ctx, |ui| {
            ui.label("Warframe Market Ducats Buyer");
            ui.label("This app works thanks to:");
            ui.hyperlink("https://warframe.market/");
            ui.add_space(10.0);
            ui.label("Made by:");
            ui.hyperlink_to("FreePhoenix888", "https://github.com/FreePhoenix888");
            ui.hyperlink_to(
              "Source Code",
              "https://github.com/FreePhoenix888/warframe-market-ducats-buyer",
            );
            ui.add_space(10.0);
            ui.label("Special thanks to the Warframe community!");
          });
    }

    if self.show_all_orders {
      egui::Window::new("All Orders")
          .open(&mut self.show_all_orders)
          .resizable(true)
          .scroll2([true, true])
          .show(ctx, |ui| {
            if let Some(orders) = &self.orders {
              ui.label("Fetched Orders:");
              ui.add_space(10.0);

              let row_height = 50.0;
              let num_rows = orders.len();

              ScrollArea::vertical().auto_shrink([false, false]).show_rows(
                ui,
                row_height,
                num_rows,
                |ui, range| {
                  for i in range.start..range.end {
                    let order = &orders[i];

                    ui.group(|ui| {
                      ui.horizontal(|ui| {
                        ui.label("ID:");
                        ui.monospace(&order.id);
                      });

                      ui.horizontal(|ui| {
                        ui.label("Item:");
                        ui.monospace(order.item_name.as_deref().unwrap_or("Unknown"));
                        if let Some(item_url) = &order.item_url {
                          ui.hyperlink(format!(
                            "https://warframe.market/items/{}",
                            item_url
                          ));
                        }
                      });

                      ui.horizontal(|ui| {
                        ui.label("Price:");
                        ui.monospace(format!("{} platinum", order.platinum));
                      });

                      ui.horizontal(|ui| {
                        ui.label("Quantity:");
                        ui.monospace(order.quantity.to_string());
                      });

                      ui.horizontal(|ui| {
                        ui.label("User:");
                        ui.monospace(&order.user.ingame_name);
                        ui.label("(Status:");
                        ui.colored_label(
                          if order.user.status == "ingame" {
                            egui::Color32::GREEN
                          } else {
                            egui::Color32::RED
                          },
                          &order.user.status,
                        );
                        ui.label(")");
                      });

                      ui.separator();
                    });
                  }
                },
              );
            } else {
              ui.label("No orders fetched yet.");
            }
          });
    }


    if self.show_delete_presets_confirmation {
      let modal = egui::Modal::new(egui::Id::new("delete_presets_confirmation"))
          .show(ctx, |ui| {
            ui.set_min_width(300.0); // Sets a minimum width for the modal

            ui.vertical_centered(|ui| {
              ui.heading("Delete All Presets?");
              ui.add_space(8.0);
              ui.label("Are you sure you want to delete all presets?");
              ui.label("This action cannot be undone!");
              ui.add_space(16.0);

              ui.horizontal(|ui| {
                // Center the buttons using available space
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                  if ui.add_sized(
                    [100.0, 30.0],
                    egui::Button::new("Yes, Delete All")
                        .fill(egui::Color32::from_rgb(178, 34, 34))
                  ).clicked() {
                    self.settings_manager.delete_all_presets();
                    self.toasts.warning("All presets have been deleted");
                    self.show_delete_presets_confirmation = false;
                  }

                  ui.add_space(16.0);

                  if ui.add_sized(
                    [100.0, 30.0],
                    egui::Button::new("Cancel")
                  ).clicked() {
                    self.show_delete_presets_confirmation = false;
                  }
                });
              });
            });
          });

      if modal.should_close() {
        self.show_delete_presets_confirmation = false;
      }
    }

    self.toasts.show(ctx);
    ctx.request_repaint();
  }
}

fn main() -> eframe::Result {
  if std::env::var("RUST_LOG").is_err() {
    unsafe {
      std::env::set_var("RUST_LOG", "info");
    }
  }
  env_logger::init();
  let options = eframe::NativeOptions {
    window_builder: Some(Box::new(|builder| builder.with_maximized(true))),
    ..Default::default()
  };
  eframe::run_native(
    "Warframe Market Ducats Buyer",
    options,
    Box::new(|_cc| Ok(Box::<MyApp>::default())),
  )
}

