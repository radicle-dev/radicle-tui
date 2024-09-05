pub mod scroll {
    pub fn percent_seen(selected: usize, len: usize, page_size: usize) -> usize {
        let step = selected;
        let page_size = page_size as f64;
        let len = len as f64;

        let lines = page_size + step.saturating_sub(page_size as usize) as f64;
        let progress = (lines / len * 100.0).ceil();

        if progress > 97.0 {
            map_range((0.0, progress), (0.0, 100.0), progress) as usize
        } else {
            progress as usize
        }
    }

    pub fn percent_absolute(offset: usize, len: usize, height: usize) -> usize {
        let y = offset as f64;
        let h = height as f64;
        let t = len.saturating_sub(1) as f64;
        let v = y / (t - h) * 100_f64;

        (v as usize).clamp(0, 100)
    }

    fn map_range(from: (f64, f64), to: (f64, f64), value: f64) -> f64 {
        to.0 + (value - from.0) * (to.1 - to.0) / (from.1 - from.0)
    }
}
