// SPDX-License-Identifier: GPL-2.0-or-later
//! Linux sensors under `/sys/class/hwmon`: the CPU's temperature and an AMD graphics card's
//! temperature and load. The files are found once (their numbering can change between boots,
//! not while running) and then read as plain text.

use std::fs;
use std::path::{Path, PathBuf};

/// Where the kernel lists its sensor chips.
pub const ROOT: &str = "/sys/class/hwmon";

/// CPU sensor drivers, best first: AMD (`k10temp`, the out-of-tree `zenpower`), Intel
/// (`coretemp`), and the generic one of ARM boards.
const CPU_CHIPS: [&str; 4] = ["k10temp", "zenpower", "coretemp", "cpu_thermal"];
/// The labels of a CPU chip's whole-package temperature, best first.
const CPU_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

/// Every chip under `root` with its driver name, in path order.
fn chips(root: &Path) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let name = fs::read_to_string(dir.join("name")).ok()?;
            Some((name.trim().to_owned(), dir))
        })
        .collect();
    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
}

/// A chip's temperature input: the first of `labels` it has, else its first input.
fn temp_input(dir: &Path, labels: &[&str]) -> Option<PathBuf> {
    let mut labelled: Vec<(usize, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let n = file.strip_prefix("temp")?.strip_suffix("_label")?;
            let label = fs::read_to_string(e.path()).ok()?;
            let rank = labels.iter().position(|l| *l == label.trim())?;
            Some((rank, dir.join(format!("temp{n}_input"))))
        })
        .filter(|(_, p)| p.exists())
        .collect();
    labelled.sort();
    labelled
        .into_iter()
        .next()
        .map(|(_, p)| p)
        .or_else(|| Some(dir.join("temp1_input")).filter(|p| p.exists()))
}

/// The CPU temperature's input file.
pub fn cpu_temp(root: &Path) -> Option<PathBuf> {
    let found = chips(root);
    CPU_CHIPS.iter().find_map(|want| {
        found
            .iter()
            .filter(|(name, _)| name == want)
            .find_map(|(_, dir)| temp_input(dir, &CPU_LABELS))
    })
}

/// An AMD graphics card: its temperature input and, when the driver gives it, its load file.
#[derive(Debug, PartialEq)]
pub struct AmdGpu {
    pub temp: PathBuf,
    pub load: Option<PathBuf>,
}

/// The AMD card with the most video memory (the add-in card rather than a processor's own
/// graphics when both are there).
pub fn amd_gpu(root: &Path) -> Option<AmdGpu> {
    chips(root)
        .into_iter()
        .filter(|(name, _)| name == "amdgpu")
        .filter_map(|(_, dir)| {
            let temp = temp_input(&dir, &["edge"])?;
            let device = dir.join("device");
            let vram = read_number(&device.join("mem_info_vram_total")).unwrap_or(0.0);
            let load = Some(device.join("gpu_busy_percent")).filter(|p| p.exists());
            Some((vram, AmdGpu { temp, load }))
        })
        .fold(
            None,
            |best: Option<(f64, AmdGpu)>, (vram, gpu)| match best {
                Some((v, _)) if v >= vram => best,
                _ => Some((vram, gpu)),
            },
        )
        .map(|(_, gpu)| gpu)
}

fn read_number(path: &Path) -> Option<f64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// A temperature input, in °C (the file holds millidegrees).
pub fn read_temp(path: &Path) -> Option<f64> {
    read_number(path).map(|m| m / 1000.0)
}

/// A load file, in percent.
pub fn read_percent(path: &Path) -> Option<f64> {
    read_number(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake `/sys/class/hwmon` under a fresh temp directory.
    struct Fake(PathBuf);

    impl Fake {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("freergb-hwmon-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Fake(dir)
        }
        fn file(&self, rel: &str, text: &str) -> &Self {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
            self
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_cpu_temperature_is_the_package_sensor_of_the_cpu_chip() {
        let f = Fake::new("cpu");
        f.file("hwmon0/name", "nvme\n")
            .file("hwmon0/temp1_input", "41000\n")
            .file("hwmon1/name", "k10temp\n")
            .file("hwmon1/temp1_label", "Tctl\n")
            .file("hwmon1/temp1_input", "55250\n")
            .file("hwmon1/temp3_label", "Tccd1\n")
            .file("hwmon1/temp3_input", "50000\n");
        let p = cpu_temp(&f.0).unwrap();
        assert_eq!(p, f.0.join("hwmon1/temp1_input"));
        assert_eq!(read_temp(&p), Some(55.25));

        // Intel: the package, not the first core.
        let i = Fake::new("intel");
        i.file("hwmon4/name", "coretemp\n")
            .file("hwmon4/temp2_label", "Core 0\n")
            .file("hwmon4/temp2_input", "48000\n")
            .file("hwmon4/temp1_label", "Package id 0\n")
            .file("hwmon4/temp1_input", "52000\n");
        assert_eq!(cpu_temp(&i.0), Some(i.0.join("hwmon4/temp1_input")));

        // No label: the chip's first input.
        let a = Fake::new("arm");
        a.file("hwmon0/name", "cpu_thermal\n")
            .file("hwmon0/temp1_input", "47000\n");
        assert_eq!(cpu_temp(&a.0), Some(a.0.join("hwmon0/temp1_input")));

        let none = Fake::new("none");
        none.file("hwmon0/name", "acpitz\n")
            .file("hwmon0/temp1_input", "30000\n");
        assert_eq!(cpu_temp(&none.0), None, "only CPU chips count");
        assert_eq!(cpu_temp(Path::new("/nonexistent/hwmon")), None);
    }

    #[test]
    fn the_amd_card_is_the_one_with_the_most_video_memory() {
        let f = Fake::new("amd");
        f.file("hwmon2/name", "amdgpu\n")
            .file("hwmon2/temp1_label", "edge\n")
            .file("hwmon2/temp1_input", "40000\n")
            .file("hwmon2/device/mem_info_vram_total", "536870912\n")
            .file("hwmon2/device/gpu_busy_percent", "3\n")
            .file("hwmon5/name", "amdgpu\n")
            .file("hwmon5/temp2_label", "junction\n")
            .file("hwmon5/temp2_input", "70000\n")
            .file("hwmon5/temp1_label", "edge\n")
            .file("hwmon5/temp1_input", "62000\n")
            .file("hwmon5/device/mem_info_vram_total", "17163091968\n")
            .file("hwmon5/device/gpu_busy_percent", "87\n");
        let gpu = amd_gpu(&f.0).unwrap();
        assert_eq!(gpu.temp, f.0.join("hwmon5/temp1_input"));
        assert_eq!(read_temp(&gpu.temp), Some(62.0));
        assert_eq!(gpu.load.as_deref().and_then(read_percent), Some(87.0));
        assert_eq!(amd_gpu(Path::new("/nonexistent/hwmon")), None);
    }
}
