//! Renders a print job to a PNG with the `typst` command line compiler.

use std::process::Command;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use serde::Deserialize;

const TEMPLATE: &str = include_str!("receipt.typ");

/// Payload of a print message
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub text: Option<String>,
    /// Interpret `text` as Typst markup instead of plain text
    #[serde(default)]
    pub markup: bool,
    /// Base64 encoded PNG, JPEG, GIF, WebP or SVG
    pub image: Option<String>,
}

impl Job {
    pub fn parse(payload: &[u8]) -> Result<Job> {
        let job: Job = serde_json::from_slice(payload).context("invalid JSON payload")?;
        if job.text.is_none() && job.image.is_none() {
            bail!("message has neither text nor image");
        }
        Ok(job)
    }
}

/// A rendered page, 80 mm wide
pub struct Page {
    pub png: Vec<u8>,
    pub height_px: u32,
    pub ppi: u32,
}

impl Page {
    /// Page height in hundredths of millimeters, rounded up so nothing is cut off
    pub fn height_hmm(&self) -> i32 {
        (u64::from(self.height_px) * 2540).div_ceil(u64::from(self.ppi)) as i32
    }
}

pub fn render(job: &Job, ppi: u32) -> Result<Page> {
    let dir = tempfile::tempdir().context("creating job directory")?;
    let mut cmd = Command::new("typst");
    cmd.arg("compile").arg("--root").arg(dir.path()).args([
        "--format",
        "png",
        "--ppi",
        &ppi.to_string(),
    ]);

    if let Some(text) = &job.text {
        std::fs::write(dir.path().join("text.txt"), text)?;
        cmd.args(["--input", "text=text.txt"]);
        if job.markup {
            cmd.args(["--input", "markup=1"]);
        }
    }
    if let Some(image) = &job.image {
        let data = base64::engine::general_purpose::STANDARD
            .decode(image.trim())
            .context("image is not valid base64")?;
        let name = format!("image.{}", image_extension(&data)?);
        std::fs::write(dir.path().join(&name), data)?;
        cmd.args(["--input", &format!("image={name}")]);
    }

    let source = dir.path().join("receipt.typ");
    let output = dir.path().join("receipt.png");
    std::fs::write(&source, TEMPLATE)?;
    let result = cmd
        .arg(&source)
        .arg(&output)
        .output()
        .context("running typst")?;
    if !result.status.success() {
        bail!(
            "typst failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    // With `height: auto` the receipt is always a single page
    let png = std::fs::read(&output).context("typst produced no output")?;
    let (_, height_px) = png_dimensions(&png).context("typst output is not a PNG")?;
    Ok(Page {
        png: set_png_ppi(&png, ppi),
        height_px,
        ppi,
    })
}

/// Typst picks the image format from the file extension
fn image_extension(data: &[u8]) -> Result<&'static str> {
    Ok(match data {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xff, 0xd8, 0xff, ..] => "jpg",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "webp",
        _ if looks_like_svg(data) => "svg",
        _ => bail!("unsupported image format (PNG, JPEG, GIF, WebP or SVG)"),
    })
}

fn looks_like_svg(data: &[u8]) -> bool {
    let head = &data[..data.len().min(1024)];
    let head = String::from_utf8_lossy(head);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    head.starts_with('<') && head.contains("<svg")
}

fn png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
        return None;
    }
    let be = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
    Some((be(16), be(20)))
}

/// Typst writes no resolution into the PNG. PAPPL needs it to print the image
/// pixel for pixel (`print-scaling=none`), so add a pHYs chunk after IHDR.
fn set_png_ppi(png: &[u8], ppi: u32) -> Vec<u8> {
    const IHDR_END: usize = 8 + 4 + 4 + 13 + 4;
    let ppm = (f64::from(ppi) / 0.0254).round() as u32;

    let mut chunk = Vec::with_capacity(21);
    chunk.extend(9u32.to_be_bytes());
    chunk.extend(b"pHYs");
    chunk.extend(ppm.to_be_bytes());
    chunk.extend(ppm.to_be_bytes());
    chunk.push(1); // unit: meter
    let crc = crc32(&chunk[4..]);
    chunk.extend(crc.to_be_bytes());

    let mut out = Vec::with_capacity(png.len() + chunk.len());
    out.extend(&png[..IHDR_END]);
    out.extend(chunk);
    out.extend(&png[IHDR_END..]);
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_payload() {
        let job = Job::parse(br#"{"text":"hi","markup":true}"#).unwrap();
        assert_eq!(job.text.as_deref(), Some("hi"));
        assert!(job.markup);
        assert!(Job::parse(b"{}").is_err());
        assert!(Job::parse(br#"{"txt":"typo"}"#).is_err());
        assert!(Job::parse(b"plain text").is_err());
    }

    #[test]
    fn formats() {
        assert_eq!(image_extension(b"\x89PNG\r\n\x1a\n").unwrap(), "png");
        assert_eq!(image_extension(b"\xff\xd8\xff\xe0").unwrap(), "jpg");
        assert_eq!(image_extension(b"RIFF\0\0\0\0WEBPVP8 ").unwrap(), "webp");
        assert_eq!(
            image_extension(b"<?xml version=\"1.0\"?>\n<svg>").unwrap(),
            "svg"
        );
        assert!(image_extension(b"hello").is_err());
    }

    #[test]
    fn crc() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
    }

    /// Needs `typst` in $PATH
    #[test]
    #[ignore]
    fn render_text_and_image() {
        // 1x1 black PNG
        let image = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg==";
        let job = Job::parse(
            format!(r#"{{"text":"Hello\n*not bold* #x","image":"{image}"}}"#).as_bytes(),
        )
        .unwrap();
        let page = render(&job, 203).unwrap();
        let (width, height) = png_dimensions(&page.png).unwrap();
        // 80 mm at 203 ppi
        assert!((638..=640).contains(&width), "width {width}");
        // the image is scaled to the full 72 mm content width, so it is square
        assert!(height > 72 * 203 * 10 / 254, "height {height}");
        assert!(page.png.windows(4).any(|w| w == b"pHYs"));
    }
}
