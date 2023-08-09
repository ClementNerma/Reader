use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread::JoinHandle, borrow::Cow,
};

use anyhow::{anyhow, bail, Context as _, Result};
use egui::{Context, InputState, RichText, Color32, Label, Area, Align2, Vec2, Key, CentralPanel, Frame, Window, Ui, Layout, Align, Spinner};
use egui_extras::RetainedImage;
use rfd::FileDialog;

use crate::{
    gap_vec::GapVec,
    img_sources::{load_image_source, ImageSource, EmptySource},
    settings::Settings,
    show_err_dialog, LOGICAL_CORES,
};

pub struct ReaderApp {
    ctx: Context,

    thread_handles: Vec<JoinHandle<()>>,
    threads_stop_signal: Arc<AtomicBool>,

    settings: Arc<RwLock<Settings>>,

    path: Option<PathBuf>,
    total_pages: usize,

    loaded_pages: Arc<RwLock<GapVec<Result<RetainedImage>>>>,
    page: usize,

    page_prompt: Option<String>,
}

impl ReaderApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        path: Option<PathBuf>,
        // settings: Settings,
    ) -> Result<Self> {
        let settings = match cc.storage {
            Some(storage) => eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default(),
            None => Settings::default(),
        };

        Ok(Self::create(
            cc.egui_ctx.clone(),
            match path {
                Some(ref path) => load_image_source(path)?,
                None => Box::new(EmptySource::new())
            },
            path,
            Arc::new(RwLock::new(settings)),
        ))
    }

    fn create(
        ctx: Context,
        img_source: Box<dyn ImageSource>,
        path: Option<PathBuf>,
        settings: Arc<RwLock<Settings>>,
    ) -> Self {
        let total_pages = img_source.total_pages();

        let loaded_pages = Arc::new(RwLock::new(GapVec::new(img_source.total_pages())));
        
        let threads_stop_signal = Arc::new(AtomicBool::new(false));
        
        let mut thread_handles = vec![];

        let threads_count = *LOGICAL_CORES;

        for thread_num in 0..threads_count {
            let mut img_source = img_source.quick_clone();

            let path = path.clone();
            let ctx_bis = ctx.clone();
            let thread_stop_signal_bis = Arc::clone(&threads_stop_signal);
            let loaded_pages_bis = Arc::clone(&loaded_pages);

            thread_handles.push(std::thread::spawn(move || {
                let total_pages = img_source.total_pages();

                let mut load_page = |page: usize| -> Result<RetainedImage> {
                    let img_debug_name = format!("{}:[page {}]", match path {
                        Some(ref path) => path.to_string_lossy(),
                        None => Cow::Borrowed("<no path>")
                    }, page);

                    let img_bytes = img_source.load_page(page)?;

                    let img = RetainedImage::from_image_bytes(img_debug_name, &img_bytes)
                        .map_err(|err| anyhow!("Failed to decode image: {err}"))?;

                    Ok(img)
                };

                // TODO: parallel (4 threads?)
                for page in 0..total_pages {
                    if page % threads_count != thread_num {
                        continue;
                    }

                    if thread_stop_signal_bis.load(Ordering::Acquire) {
                        return;
                    }

                    let img = load_page(page);

                    loaded_pages_bis.write().unwrap().set(page, img);

                    ctx_bis.request_repaint();
                }
            }));
    }

        Self {
            ctx,
            thread_handles,
            threads_stop_signal,
            path,
            loaded_pages,
            settings,
            total_pages,
            page: 0,
            page_prompt: None,
        }
    }

    fn load_path(&mut self, path: PathBuf) -> Result<()> {
        let img_source = load_image_source(&path)?;

        self.threads_stop_signal.store(true, Ordering::Release);

        
        while let Some(thread_handle) = self.thread_handles.pop() {
            thread_handle.join().map_err(|_| anyhow!("Internal error: failed to join thread"))?;
        }

        *self = Self::create(
            self.ctx.clone(),
            img_source,
            Some(path),
            Arc::clone(&self.settings),
        );

        Ok(())
    }

    fn relative_file_change(&mut self, relative: isize) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };

        let Some(parent) = path.parent() else {
            return Ok(())
        };

        let items = fs::read_dir(parent)?.collect::<Result<Vec<_>, _>>()?;

        let index = items
            .iter()
            .position(|c| &c.path() == path)
            .context("File not found in parent directory")?;

        if -relative > isize::try_from(index).unwrap() {
            bail!("No previous file in parent directory");
        }

        let index = usize::try_from(isize::try_from(index).unwrap() + relative).unwrap();

        if index >= items.len() {
            bail!("No next file in parent directory");
        }

        self.load_path(items[index].path())
    }

    fn relative_page_change(&mut self, mut inc: isize, shift: bool) {
        let settings = self.settings.read().unwrap();

        if settings.double_page && !shift {
            inc *= 2;
        }

        // if settings.right_to_left {
        //     inc *= -1;
        // }

        if inc < 0 {
            let dec = usize::try_from(-inc).unwrap();
            self.page = if dec >= self.page { 0 } else { self.page - dec };
        } else {
            let c_page = self.page + usize::try_from(inc).unwrap();
            let max_page = if self.total_pages == 0 {
                0
            } else {
                self.total_pages - 1
            };

            self.page = std::cmp::min(c_page, max_page);
        }
    }

    fn handle_inputs(&mut self, i: &InputState) {
        if i.key_pressed(Key::Home) {
            self.page = 0;
        }

        if i.key_pressed(Key::End) {
            self.page = if self.total_pages <= 1 {
                0
            } else if self.settings.read().unwrap().double_page {
                self.total_pages - 2
            } else {
                self.total_pages - 1
            };
        }

        if i.key_pressed(Key::ArrowLeft) || i.scroll_delta.x >= 50.0 || i.scroll_delta.y >= 50.0 {
            if i.modifiers.ctrl {
                if let Err(err) = self.relative_file_change(-1) {
                    show_err_dialog(err);
                }
            } else {
                self.relative_page_change(-1, i.modifiers.shift);
            }
        }

        if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::Space) || i.scroll_delta.x <= -50.0 || i.scroll_delta.y <= -50.0 {
            if i.modifiers.ctrl {
                if let Err(err) = self.relative_file_change(1) {
                    show_err_dialog(err);
                }
            } else {
                self.relative_page_change(1, i.modifiers.shift);
            }
        }

        if i.key_pressed(Key::O) && i.modifiers.ctrl {
            let mut dialog = FileDialog::new().add_filter("comics", &["zip", "cbz"]);

            if let Some(parent_dir) = self.path.as_ref().and_then(|path| path.parent()) {
                dialog = dialog.set_directory(parent_dir);
            }

            let item = if i.modifiers.shift {
                dialog.pick_folder()
            } else {
                dialog.pick_file()
            };

            if let Some(item) = item {
                if let Err(err) = self.load_path(item) {
                    show_err_dialog(err);
                }
            }
        }

        if i.key_pressed(Key::D) {
            let mut settings = self.settings.write().unwrap();
            settings.double_page = !settings.double_page;
        }

        if i.key_pressed(Key::R) {
            let mut settings = self.settings.write().unwrap();
            settings.right_to_left = !settings.right_to_left;
        }

        if i.key_pressed(Key::I) {
            let mut settings = self.settings.write().unwrap();
            settings.display_pages_number = !settings.display_pages_number;
        }

        if i.key_pressed(Key::Escape) {
            std::process::exit(0);
        }

        if i.key_pressed(Key::G) {
            self.page_prompt = Some(String::new());
        }
    }

    fn handle_file_drops(&mut self, i: &InputState) {
        let files = &i.raw.dropped_files;

        if files.is_empty() {
           return; 
        }

        if files.len() > 1 {
            return show_err_dialog(anyhow!("Please drop only one item"));
        }

        let file = files.get(0).unwrap();

        let Some(path) = &file.path else {
            return show_err_dialog(anyhow!("Dropped file must be a file stored on disk"));
        };

        if let Err(err) = self.load_path(path.to_owned()) {
            show_err_dialog(err);
        }
    }
}

impl eframe::App for ReaderApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &*self.settings.read().unwrap());
    }

    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        CentralPanel::default()
            .frame(Frame::none())
            .show(ctx, |ui| {
                ctx.input(|i| {
                    self.handle_inputs(i);
                    self.handle_file_drops(i);
                });

                let win_size = frame.info().window_info.size;

                if self.page_prompt.is_some() {
                    Window::new("Jump to page")
                        .pivot(Align2::CENTER_CENTER)
                        .default_pos((win_size / 2.0).to_pos2())
                        .show(&self.ctx, |ui| {
                            ui.label("Jump to page:");

                            ui.text_edit_singleline(self.page_prompt.as_mut().unwrap());

                            ui.horizontal(|ui| {
                                if ui.button("OK").clicked() {
                                    let Ok(page) = self.page_prompt.as_ref().unwrap().parse::<usize>() else {
                                        return show_err_dialog(anyhow!("Invalid page number provided"));                                    
                                    };

                                    if page == 0 {
                                        return show_err_dialog(anyhow!("Invalid page number provided"));
                                    }

                                    if page > self.total_pages {
                                        return show_err_dialog(anyhow!("Book only contains {} pages", self.total_pages));
                                    }

                                    self.page = page - 1;
                                    self.page_prompt = None;
                                }

                                if ui.button("Cancel").clicked() {
                                    self.page_prompt = None;
                                }
                            });
                        });
                }

                let settings = self.settings.read().unwrap();

                let render_page = |ui: &mut Ui, page: usize| {
                    if page >= self.total_pages {
                        ui.label(" "); // Empty widget
                    } else {
                        match self.loaded_pages.read().unwrap().get(page) {
                            None => {
                                ui.heading("Loading...");
                                ui.add(Spinner::new());
                            }

                            Some(page_img) => match page_img {
                                Ok(page_img) => {
                                    let scale = frame.info().window_info.size.y
                                        / f32::from(u16::try_from(page_img.height()).unwrap());

                                    page_img.show_scaled(ui, scale);
                                }

                                Err(err) => {
                                    ui.heading(format!("Failed to load: {err}"));
                                }
                            },
                        }
                    }
                };

                let pages = if self.total_pages == 0 {
                    ui.heading("Nothing to display");
                    
                    (None, None)
                } else if !settings.double_page || self.page + 1 == self.total_pages {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        render_page(ui, self.page);
                    });

                    (Some(self.page), None)
                } else {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.columns(2, |columns| {
                        let (left_page, right_page) = if settings.right_to_left {
                            (self.page + 1, self.page)
                        } else {
                            (self.page, self.page + 1)
                        };

                        columns[0].with_layout(
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                render_page(ui, left_page);
                            },
                        );

                        columns[1].with_layout(
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                render_page(ui, right_page);
                            },
                        );
                    });

                    (Some(self.page), Some(self.page + 1))
                };

                if settings.display_pages_number {
                    Area::new("pages_number")
                        .anchor(Align2::RIGHT_TOP, Vec2::ZERO)
                        .show(ctx, |ui| {
                            let text = format!(
                                "{}/{}",
                                match pages {
                                    (None, None) => "-".to_string(),
                                    (Some(left), None) => (left + 1).to_string(),
                                    (Some(left), Some(right)) => format!("{}-{}", left + 1, right + 1),
                                    (None, Some(_)) => unreachable!()
                                },
                                self.total_pages
                            );

                            ui.add(Label::new(RichText::from(text).heading().background_color(Color32::BLACK)).wrap(false));
                        });
                }
            });
    }
}
