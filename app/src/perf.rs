//! Opt-in native-window observations. No frame-path file I/O; report on exit.
use nana_ui::runtime::RuntimeDocument;
use nana_ui_platform::WindowId;
use std::{collections::HashMap, path::PathBuf, time::Instant};

#[derive(Default)]
struct Window {
    prepared: Option<Instant>,
    presented: u64,
    outputs: u64,
    presented_after_6s: u64,
    outputs_after_6s: u64,
    revision: Option<u64>,
    text_shaped: u64,
    layout_nodes: u64,
    draw_calls: Option<u64>,
    upload_bytes: Option<u64>,
    samples: Vec<f64>,
}

pub struct Recorder {
    path: PathBuf,
    started: Instant,
    windows: HashMap<WindowId, Window>,
}
impl Recorder {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            path: std::env::var_os("NANATYPE_PERF").map(PathBuf::from)?,
            started: Instant::now(),
            windows: HashMap::new(),
        })
    }
    pub fn begin(&mut self, id: WindowId) {
        self.windows.entry(id).or_default().prepared = Some(Instant::now());
    }
    pub fn frame(&mut self, id: WindowId, output: bool, doc: Option<&RuntimeDocument>) {
        // Discard startup and initial pipeline compilation consistently.
        if self.started.elapsed().as_millis() < 2000 {
            return;
        }
        let window = self.windows.entry(id).or_default();
        if output {
            window.outputs += 1;
            window.outputs_after_6s += u64::from(self.started.elapsed().as_millis() >= 6000);
        } else {
            window.presented += 1;
            window.presented_after_6s += u64::from(self.started.elapsed().as_millis() >= 6000);
        }
        if let Some(start) = window.prepared.take() {
            if window.samples.len() < 10_000 {
                window.samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            if let Some(doc) = doc {
                let cx = doc.context();
                let work = cx.last_work_counters();
                let revision = cx.world().pending_work_revision();
                if window.revision != Some(revision) {
                    window.revision = Some(revision);
                    window.text_shaped += work.text_shaped as u64;
                    window.layout_nodes += work.layout_nodes as u64;
                }
                sum(&mut window.draw_calls, work.draw_calls);
                sum(&mut window.upload_bytes, work.gpu_upload_bytes);
            }
        }
    }
}
fn sum(total: &mut Option<u64>, value: Option<usize>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0) + value as u64);
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        let windows:Vec<_>=self.windows.iter_mut().map(|(id,w)| {
            w.samples.sort_by(f64::total_cmp);
            let median=(!w.samples.is_empty()).then(||w.samples[w.samples.len()/2]);
            let p95=(!w.samples.is_empty()).then(||w.samples[w.samples.len()*95/100]);
            serde_json::json!({"window":format!("{:?}",crate::windows::WindowKind::from_id(*id)),"presented_frames":w.presented,"output_frames":w.outputs,
                "presented_after_6s":w.presented_after_6s,"output_after_6s":w.outputs_after_6s,
                "prepare_to_callback_median_ms":median,"prepare_to_callback_p95_ms":p95,
                "text_shaped":w.text_shaped,"layout_nodes":w.layout_nodes,"draw_calls":w.draw_calls,"gpu_upload_bytes":w.upload_bytes})
        }).collect();
        let report = serde_json::json!({"schema":1,"warmup_ms":2000,"elapsed_ms":self.started.elapsed().as_millis(),"windows":windows,
            "measurement":"Native frame/output callbacks. Prepare-to-callback is host wall time, not a GPU timestamp. Missing GPU counters are null. See gpu_perf for GPU timestamps."});
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_vec_pretty(&report) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}
