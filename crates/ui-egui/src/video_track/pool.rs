//! Movie decoding jobs, worker lifetime and the UI texture cache.

use egui::Color32;
use soundcraft_model::SourceId;
use soundcraft_video::Frame;
use soundcraft_video::Movie;
use soundcraft_video::MovieInfo;
use std::collections::HashMap;
use std::sync::Arc;

/// Most thumbnail textures kept; beyond that the cache is cleared and refilled on demand.
const MAX_THUMBS: usize = 600;

/// Identity of a movie: session source id plus a hash of its path (ids repeat across sessions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MovieKey(pub SourceId, pub u64);

impl MovieKey {
    pub fn new(id: SourceId, path: &str) -> MovieKey {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut h);
        MovieKey(id, h.finish())
    }
}

/// A thumbnail request key: movie, time (ms) and pixel height.
type ThumbKey = (MovieKey, i64, u32);

enum Job {
    Frame { key: MovieKey, path: String, secs: f64 },
    Thumb { key: MovieKey, path: String, secs: f64, h: u32, tk: ThumbKey },
}

enum Done {
    Info(MovieKey, Result<MovieInfo, String>),
    Frame(MovieKey, f64, Result<Arc<Frame>, String>),
    Thumb(ThumbKey, Result<Arc<Frame>, String>),
}

/// Owns the open movies; runs jobs (on the worker thread, or inline on wasm).
#[derive(Default)]
struct Worker {
    movies: HashMap<MovieKey, Result<Movie, String>>,
}

impl Worker {
    fn open(&mut self, key: MovieKey, path: &str, out: &mut Vec<Done>) -> Option<&mut Movie> {
        if !self.movies.contains_key(&key) {
            let m = open_movie(path);
            out.push(Done::Info(key, m.as_ref().map(|m| m.info().clone()).map_err(Clone::clone)));
            // Keep a handful of movies open.
            if self.movies.len() >= 8 {
                self.movies.clear();
            }
            self.movies.insert(key, m);
        }
        self.movies.get_mut(&key).and_then(|m| m.as_mut().ok())
    }

    fn run(&mut self, job: Job) -> Vec<Done> {
        let mut out = Vec::new();
        match job {
            Job::Frame { key, path, secs } => {
                let r = match self.open(key, &path, &mut out) {
                    Some(m) => m.frame_at(secs).map_err(|e| e.to_string()),
                    None => Err("movie offline".to_string()),
                };
                out.push(Done::Frame(key, secs, r));
            }
            Job::Thumb { key, path, secs, h, tk } => {
                let r = match self.open(key, &path, &mut out) {
                    Some(m) => m.thumbnail_at(secs, h).map_err(|e| e.to_string()),
                    None => Err("movie offline".to_string()),
                };
                out.push(Done::Thumb(tk, r));
            }
        }
        out
    }
}

fn open_movie(path: &str) -> Result<Movie, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let f = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
        Movie::open_source(Arc::new(f)).map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        Movie::open(bytes).map_err(|e| e.to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use super::{Done, Job, Worker};
    use std::collections::VecDeque;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::sync::{Arc, Condvar, Mutex};

    #[derive(Default)]
    struct Queue {
        frame: Option<Job>,
        thumbs: VecDeque<Job>,
        quit: bool,
    }

    /// The decoding thread and its job queue.
    pub struct Backend {
        shared: Arc<(Mutex<Queue>, Condvar)>,
        rx: Receiver<Done>,
    }

    impl Backend {
        pub fn new(ctx: egui::Context) -> Option<Backend> {
            let shared = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
            let (tx, rx) = channel();
            let s2 = Arc::clone(&shared);
            std::thread::Builder::new().name("video-decode".into()).spawn(move || run(&s2, &tx, &ctx)).ok()?;
            Some(Backend { shared, rx })
        }

        pub fn submit(&mut self, job: Job) {
            let (lock, cv) = &*self.shared;
            if let Ok(mut q) = lock.lock() {
                match job {
                    Job::Frame { .. } => q.frame = Some(job),
                    Job::Thumb { .. } => {
                        q.thumbs.push_back(job);
                        // Stale requests (scrolled away) are dropped; their slots are freed so
                        // they can be asked again.
                        while q.thumbs.len() > 96 {
                            q.thumbs.pop_front();
                        }
                    }
                }
                cv.notify_one();
            }
        }

        /// Finished jobs.
        pub fn poll(&mut self) -> Vec<Done> {
            self.rx.try_iter().collect()
        }

        /// Thumbnail jobs still waiting (pending slots not listed here were dropped or are running).
        pub fn queued_thumbs(&self) -> Vec<super::ThumbKey> {
            let (lock, _) = &*self.shared;
            lock.lock()
                .map(|q| q.thumbs.iter().filter_map(|j| if let Job::Thumb { tk, .. } = j { Some(*tk) } else { None }).collect())
                .unwrap_or_default()
        }
    }

    impl Drop for Backend {
        fn drop(&mut self) {
            let (lock, cv) = &*self.shared;
            if let Ok(mut q) = lock.lock() {
                q.quit = true;
            }
            cv.notify_all();
        }
    }

    fn run(shared: &(Mutex<Queue>, Condvar), tx: &Sender<Done>, ctx: &egui::Context) {
        let mut worker = Worker::default();
        loop {
            let job = {
                let (lock, cv) = shared;
                let Ok(mut q) = lock.lock() else { return };
                while q.frame.is_none() && q.thumbs.is_empty() && !q.quit {
                    q = match cv.wait(q) {
                        Ok(q) => q,
                        Err(_) => return,
                    };
                }
                if q.quit {
                    return;
                }
                match q.frame.take() {
                    Some(j) => j,
                    None => match q.thumbs.pop_back() {
                        // Newest first: what is on screen now.
                        Some(j) => j,
                        None => continue,
                    },
                }
            };
            // A decoder bug must not take the app down: contain it to this job.
            let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.run(job)));
            match done {
                Ok(done) => {
                    for d in done {
                        if tx.send(d).is_err() {
                            return;
                        }
                    }
                }
                Err(_) => worker = Worker::default(),
            }
            ctx.request_repaint();
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use super::{Done, Job, Worker};
    use std::collections::VecDeque;

    /// Inline decoding with a per-frame budget (no threads on the web).
    pub struct Backend {
        worker: Worker,
        frame: Option<Job>,
        thumbs: VecDeque<Job>,
        ctx: egui::Context,
    }

    impl Backend {
        pub fn new(ctx: egui::Context) -> Option<Backend> {
            Some(Backend { worker: Worker::default(), frame: None, thumbs: VecDeque::new(), ctx })
        }
        pub fn submit(&mut self, job: Job) {
            match job {
                Job::Frame { .. } => self.frame = Some(job),
                Job::Thumb { .. } => {
                    self.thumbs.push_back(job);
                    while self.thumbs.len() > 96 {
                        self.thumbs.pop_front();
                    }
                }
            }
        }
        /// Run at most one frame and two thumbnails per UI frame.
        pub fn poll(&mut self) -> Vec<Done> {
            let mut out = Vec::new();
            if let Some(j) = self.frame.take() {
                out.extend(self.worker.run(j));
            }
            for _ in 0..2 {
                let Some(j) = self.thumbs.pop_back() else { break };
                out.extend(self.worker.run(j));
            }
            if !self.thumbs.is_empty() {
                self.ctx.request_repaint();
            }
            out
        }
        pub fn queued_thumbs(&self) -> Vec<super::ThumbKey> {
            self.thumbs.iter().filter_map(|j| if let Job::Thumb { tk, .. } = j { Some(*tk) } else { None }).collect()
        }
    }
}

enum Slot {
    /// Requested at this UI time (seconds).
    Pending(f64),
    Ready(egui::TextureHandle),
    Failed,
}

/// The UI's movie cache: decoded pictures as textures (never serialised; rebuilt from paths).
#[derive(Default)]
pub struct VideoPool {
    backend: Option<backend::Backend>,
    backend_failed: bool,
    infos: HashMap<MovieKey, Result<MovieInfo, String>>,
    thumbs: HashMap<ThumbKey, Slot>,
    /// The latest playhead picture: movie, its frame index, texture.
    current: Option<(MovieKey, usize, egui::TextureHandle)>,
    current_err: Option<(MovieKey, String)>,
    /// Last frame request (to avoid re-sending identical requests).
    last_req: Option<(MovieKey, i64)>,
    /// Pictures shown in the Video window since start (diagnostics).
    pub frames_shown: u64,
}

impl VideoPool {
    fn backend(&mut self, ctx: &egui::Context) -> Option<&mut backend::Backend> {
        if self.backend.is_none() && !self.backend_failed {
            self.backend = backend::Backend::new(ctx.clone());
            self.backend_failed = self.backend.is_none();
        }
        self.backend.as_mut()
    }

    /// Collect finished work (call once per frame before drawing).
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(b) = self.backend.as_mut() else { return };
        let done = b.poll();
        let queued: std::collections::HashSet<ThumbKey> = b.queued_thumbs().into_iter().collect();
        for d in done {
            match d {
                Done::Info(k, info) => {
                    self.infos.insert(k, info);
                }
                Done::Frame(k, _secs, r) => match r {
                    Ok(f) => {
                        let img = color_image(&f);
                        match &mut self.current {
                            Some((ck, ci, tex)) => {
                                tex.set(img, egui::TextureOptions::LINEAR);
                                *ck = k;
                                *ci = f.index;
                            }
                            None => self.current = Some((k, f.index, ctx.load_texture("video-frame", img, egui::TextureOptions::LINEAR))),
                        }
                        self.current_err = None;
                        self.frames_shown += 1;
                    }
                    Err(e) => self.current_err = Some((k, e)),
                },
                Done::Thumb(tk, r) => {
                    let slot = match r {
                        Ok(f) => Slot::Ready(ctx.load_texture(format!("video-thumb-{tk:?}"), color_image(&f), egui::TextureOptions::LINEAR)),
                        Err(_) => Slot::Failed,
                    };
                    self.thumbs.insert(tk, slot);
                }
            }
        }
        // Pending slots whose job fell off the queue (scrolled away) may be asked for again; the
        // one being decoded right now is not queued either, so give it a moment.
        let now = ctx.input(|i| i.time);
        self.thumbs.retain(|k, s| !matches!(s, Slot::Pending(t) if now - *t > 2.0 && !queued.contains(k)));
    }

    pub fn info(&self, key: MovieKey) -> Option<&Result<MovieInfo, String>> {
        self.infos.get(&key)
    }

    /// Ask for the picture at `secs` of the movie (the Video window).
    pub fn request_frame(&mut self, ctx: &egui::Context, key: MovieKey, path: &str, secs: f64, fps: f64) {
        let q = (secs * fps.max(1.0) * 4.0).floor() as i64;
        if self.last_req == Some((key, q)) {
            return;
        }
        self.last_req = Some((key, q));
        let path = path.to_string();
        if let Some(b) = self.backend(ctx) {
            b.submit(Job::Frame { key, path, secs });
        }
    }

    /// The current playhead picture for `key`, if one has been decoded.
    pub fn current(&self, key: MovieKey) -> Option<&egui::TextureHandle> {
        self.current.as_ref().filter(|c| c.0 == key).map(|c| &c.2)
    }

    pub fn current_error(&self, key: MovieKey) -> Option<&str> {
        self.current_err.as_ref().filter(|c| c.0 == key).map(|c| c.1.as_str())
    }

    /// A thumbnail at `secs`, `h` pixels high; requested in the background when missing.
    pub fn thumb(&mut self, ctx: &egui::Context, key: MovieKey, path: &str, secs: f64, h: u32) -> Option<egui::TextureHandle> {
        let tk = (key, (secs * 1000.0).round() as i64, h);
        match self.thumbs.get(&tk) {
            Some(Slot::Ready(t)) => return Some(t.clone()),
            Some(Slot::Pending(_) | Slot::Failed) => return None,
            None => {}
        }
        if self.thumbs.len() >= MAX_THUMBS {
            self.thumbs.retain(|_, s| matches!(s, Slot::Pending(_)));
        }
        self.thumbs.insert(tk, Slot::Pending(ctx.input(|i| i.time)));
        let path = path.to_string();
        if let Some(b) = self.backend(ctx) {
            b.submit(Job::Thumb { key, path, secs, h, tk });
        }
        None
    }
}

fn color_image(f: &Frame) -> egui::ColorImage {
    let (w, h) = (f.width as usize, f.height as usize);
    if f.rgba.len() == w * h * 4 {
        egui::ColorImage::from_rgba_unmultiplied([w, h], &f.rgba)
    } else {
        egui::ColorImage::new([1, 1], vec![Color32::BLACK])
    }
}
