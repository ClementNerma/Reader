use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering, AtomicUsize},
        Arc, RwLock,
    },
    thread::JoinHandle, cell::RefCell,
};

use anyhow::{anyhow, bail, Context as _, Result};
use egui::{Context, InputState, RichText, Color32, Label, Area, Align2, Vec2, Key, CentralPanel, Frame, Window, Ui, Layout, Align, Spinner,  TextureOptions, ColorImage, vec2, TextureHandle};
use rfd::FileDialog;

use crate::{
    gap_vec::GapVec,
    sources::{load_image_source, ImageSource, EmptySource},
    settings::Settings,
    show_err_dialog, LOGICAL_CORES, decoders::{decode_image, DecodedImage},
};

type PageLoadingResult = Result<(PathBuf, Vec<u8>), String>;

pub struct ReaderApp {
    /// [`egui`]'s context
    ctx: Context,

    /// All threads used by the application
    thread_handles: Vec<JoinHandle<()>>,

    /// Setting this signal to `true` will make all the thread stop properly
    /// This allows them to properly finish their work and quit in a non-dirty state
    threads_stop_signal: Arc<AtomicBool>,
    
    /// Application settings
    settings: Arc<RwLock<Settings>>,

    /// Path of the currently opened file or directory (None = no file is opened)
    path: Option<PathBuf>,

    /// Total number of pages in the current file
    total_pages: usize,

    /// All loaded pages (as bytes)
    loaded_pages: Arc<RwLock<GapVec<PageLoadingResult>>>,

    // This is used to allow a rendering closure to store result of the only two
    // pages we may be interested in: the left and right one (in double mode)
    //
    // When the computable image is displayed, we store it here to avoid having to
    // re-compute it on each frame
    retained_odd_page_image: RefCell<Option<(usize, TextureHandle, Vec2)>>,
    retained_even_page_image: RefCell<Option<(usize, TextureHandle, Vec2)>>,

    /// Current page number
    current_page: Arc<AtomicUsize>,

    /// Contains the "jump to page" modal's prompt (if opened)
    page_prompt: Option<String>,
}

impl ReaderApp {
    /// Set up the application
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        path: Option<PathBuf>,
    ) -> Result<Self> {
        // Load settings from the application's storage, or use default ones
        let settings = match cc.storage {
            Some(storage) => eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default(),
            None => Settings::default(),
        };

        Ok(Self::create(
            cc.egui_ctx.clone(),
            match path {
                Some(ref path) => load_image_source(path)?,
                // If no path was provided, load a dummy empty source
                None => Box::new(EmptySource::new())
            },
            path,
            Arc::new(RwLock::new(settings)),
        ))
    }

    /// Create an application with all the required data
    fn create(
        ctx: Context,
        img_source: Box<dyn ImageSource>,
        path: Option<PathBuf>,
        settings: Arc<RwLock<Settings>>,
    ) -> Self {
        let total_pages = img_source.total_pages();
        let loaded_pages = Arc::new(RwLock::new(GapVec::new(img_source.total_pages())));
        let threads_stop_signal = Arc::new(AtomicBool::new(false));
        let current_page = Arc::new(AtomicUsize::new(0));

        // We collect here the list of all threads that we'll need to close when e.g.
        // loading another file
        let mut thread_handles = vec![];

        // How many loading threads to use
        let threads_count = std::cmp::min(*LOGICAL_CORES, 16);

        // Create the loading threads
        for thread_num in 0..threads_count {
            let mut img_source = img_source.quick_clone().unwrap();

            let ctx = ctx.clone();
            let thread_stop_signal = Arc::clone(&threads_stop_signal);
            let loaded_pages = Arc::clone(&loaded_pages);
            let current_page = Arc::clone(&current_page);

            // Each thread loads a part of the pages, depending on its number
            // The loaded pages are (total_threads * n) + thread_number
            //
            // For instance, given 8 threads:
            // Thread n°4 will load pages 4, 12, 20, etc.
            // Thread n°6 will load pages 6, 14, 22, etc.
            thread_handles.push(std::thread::spawn(move || {
                // We setup the pages to load here, this is useful when changing priorities below
                let mut pages_to_load = (0..total_pages).filter(|i| i % threads_count == thread_num).collect::<Vec<_>>();

                // Load remaining pages
                while !pages_to_load.is_empty() {
                    // The priority is always to load the pages the user is looking at first,
                    // and then the next ones in the image set.
                    // So before loading a page, we always get the first one greater than or equal to
                    // the current one.
                    let prioritize_loading_from = current_page.load(Ordering::Acquire);

                    // We get the index of the page index in the list...
                    let page_index_in_vec = pages_to_load.iter().position(|i| *i >= prioritize_loading_from).unwrap_or(0);

                    // ...to remove it and retrieve it
                    let page = pages_to_load.remove(page_index_in_vec);

                    // We load the image from the source
                    let img = img_source.load_page(page);

                    // Then we save it to the list of loaded pages
                    // Note that the lock is acquired in a single condition, meaning the lock
                    // is dropped immediatly after the writing
                    loaded_pages.write().unwrap().set(page, img);

                    // Request a repaint (will trigger the UI update function to take
                    // into account the fact we now have new pages data available)
                    ctx.request_repaint();

                    // If the application indicates it's trying to stop...
                    if thread_stop_signal.load(Ordering::Acquire) {
                        // Just quit the thread!
                        return;
                    }
                }
            }));
        }
        
        Self {
            ctx,
            thread_handles,
            threads_stop_signal,
            path,
            settings,
            total_pages,
            loaded_pages,
            retained_odd_page_image: RefCell::new(None),
            retained_even_page_image: RefCell::new(None),
            current_page,
            page_prompt: None,
        }
    }

    /// Load a new file or directory
    fn load_path(&mut self, path: PathBuf) -> Result<()> {
        // Load the image source (to ensure it's valid)
        let img_source = load_image_source(&path)?;

        // Then indicate all threads they must stop as soon as possible
        self.threads_stop_signal.store(true, Ordering::Release);

        // Wait for all threads to finish properly
        while let Some(thread_handle) = self.thread_handles.pop() {
            thread_handle.join().map_err(|_| anyhow!("Internal error: failed to join thread"))?;
        }

        // Then re-create the application (which will set up new threads)
        // NOTE: it's crucial that this function call doesn't fail (e.g. not return an error)
        //       otherwise, we'd be let with an inconsistent state (no thread to load pages)
        *self = Self::create(
            self.ctx.clone(),
            img_source,
            Some(path),
            Arc::clone(&self.settings),
        );

        Ok(())
    }

    /// Jump to a neighbour file
    fn relative_file_change(&mut self, relative: isize) -> Result<()> {
        assert!(relative == -1 || relative == 1);

        // If there is no open file, we cannot get the list of neighbour ones
        // So we don't do anything
        let Some(path) = &self.path else {
            return Ok(());
        };

        // Same goes if the opened file doesn't have a parent
        // (e.g. we opened the root directory)
        let Some(parent) = path.parent() else {
            return Ok(())
        };

        // Get all items in the current file's parent directory
        let items = fs::read_dir(parent)?.collect::<Result<Vec<_>, _>>()?;

        // Find it in the list
        // Note that it may have been moved between the moment it was opened and now
        let index = items
            .iter()
            .position(|c| &c.path() == path)
            .context("File not found in parent directory")?;

        // Check if we can do the jump
        if -relative > isize::try_from(index).unwrap() {
            bail!("No previous file in parent directory");
        }

        let index = usize::try_from(isize::try_from(index).unwrap() + relative).unwrap();

        if index >= items.len() {
            bail!("No next file in parent directory");
        }

        // Jump!
        self.load_path(items[index].path())
    }

    /// Perform a relative page change
    fn relative_page_change(&mut self, mut inc: isize, shift: bool) {
        assert!(inc == -1 || inc == 1);

        let settings = self.settings.read().unwrap();

        let current_page = self.current_page.load(Ordering::Acquire);

        if settings.double_page && !shift && (current_page != 0 || !settings.display_first_page_in_single_mode) {
            inc *= 2;
        }

        // if settings.right_to_left {
        //     inc *= -1;
        // }

        if inc < 0 {
            let dec = usize::try_from(-inc).unwrap();
            self.current_page.store(if dec >= current_page { 0 } else { current_page - dec }, Ordering::Release);
        } else {
            let c_page = current_page + usize::try_from(inc).unwrap();
            let max_page = if self.total_pages == 0 {
                0
            } else {
                self.total_pages - 1
            };

             self.current_page.store(std::cmp::min(c_page, max_page), Ordering::Release);
        }
    }

    /// Handle inputs (keyboard, mouse, etc.) from the UI thread
    fn handle_inputs(&mut self, i: &InputState) {
        if i.key_pressed(Key::Home) {
            self.current_page.store(0, Ordering::Release);
        }

        if i.key_pressed(Key::End) {
            self.current_page.store(if self.total_pages <= 1 {
                0
            } else if self.settings.read().unwrap().double_page {
                self.total_pages - 2
            } else {
                self.total_pages - 1
            }, Ordering::Release);
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

    /// Handle file drops from other applications
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

    /// Compute a displayable image for a given page
    fn compute_displayable_page(&self, page: usize) -> Result<Option<(TextureHandle, Vec2)>, String> {
        let Some(result) = self.loaded_pages.read().unwrap().get(page).cloned() else {
            return Ok(None);
        };

        let (filename, bytes) = result?;

        let DecodedImage { rgb8_pixels, width, height } = decode_image(&filename, &bytes).map_err(|err| format!("Failed to decode image: {err}"))?;

        let image = ColorImage::from_rgb([width, height], &rgb8_pixels);

        let tex_handle = self.ctx.load_texture(format!("{}:[page-{page}]", filename.to_string_lossy()), image, TextureOptions::default());

        Ok(Some((tex_handle, vec2(width as f32, height as f32))))
    }
}

impl eframe::App for ReaderApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Save settings
        eframe::set_value(storage, eframe::APP_KEY, &*self.settings.read().unwrap());
    }

    // The main rendering function, which computes the UI in immediate mode
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        // We first need a central panel to display everything inside
        CentralPanel::default()
            .frame(Frame::none())
            .show(ctx, |ui| {
                // We start by handling user inputs
                // this may impact the current page number, opened file, etc.
                ctx.input(|i| {
                    self.handle_inputs(i);
                    self.handle_file_drops(i);
                });

                // Get the current window's size (required to scale the pages properly)
                let win_size = frame.info().window_info.size;

                // If the "jump to page" modal is opened...
                if self.page_prompt.is_some() {
                    // Show it!
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

                                    self.current_page.store(page - 1, Ordering::Release);
                                    self.page_prompt = None;
                                }

                                if ui.button("Cancel").clicked() {
                                    self.page_prompt = None;
                                }
                            });
                        });
                }

                // Render a given page in the UI, synchronously
                let render_page = |ui: &mut Ui, page: usize| {
                    if page >= self.total_pages {
                        ui.label(" "); // Empty widget
                    } else {
                        let mut ptr = if page % 2 != 0 {
                            self.retained_odd_page_image.borrow_mut()
                        } else {
                            self.retained_even_page_image.borrow_mut()
                        };

                        let loaded = if let Some((_, tex_handle, size)) = ptr.as_ref().filter(|(c_page, _, _)| *c_page == page) {
                            println!("> Loaded page {page} from cache");
                            Ok(Some((tex_handle.clone(), *size)))
                        } else {
                            println!("> Computing displayable image for page {page}...");
                            self.compute_displayable_page(page)
                        };

                        match loaded {
                            Ok(data) => match data {
                                Some((tex_handle, size)) => {
                                    let scale = frame.info().window_info.size.y / size.y;
                                    ui.image(tex_handle.id(), size * scale);

                                    if ptr.is_none() {
                                        *ptr = Some((page, tex_handle, size));
                                    }
                                },
                                None => {
                                    ui.heading("Loading...");
                                    ui.add(Spinner::new());
                                },
                            },
                            Err(err) => {
                                ui.heading(format!("Failed to load page: {err}"));
                            },
                        }
                    }
                };

                let settings = self.settings.read().unwrap();

                let current_page = self.current_page.load(Ordering::Acquire);

                // Determine the pages to render and render them
                let pages = if self.total_pages == 0 {
                    ui.heading("Nothing to display");
                    
                    (None, None)
                } else if !settings.double_page || current_page + 1 == self.total_pages || (current_page == 0 && settings.display_first_page_in_single_mode) {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        render_page(ui, current_page);
                    });

                    (Some(current_page), None)
                } else {
                    // We remove any space between columns to get a gapless display in double mode
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    ui.columns(2, |columns| {
                        let (left_page, right_page) = if settings.right_to_left {
                            (current_page + 1, current_page)
                        } else {
                            (current_page, current_page + 1)
                        };

                        // Using a two-columns layout allows to use custom alignemnt
                        // for each of them

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

                    (Some(current_page), Some(current_page + 1))
                };

                // Display the pages number if enabled in the settings
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
