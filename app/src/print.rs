//! Submits rendered pages to LPrint over IPP.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use ipp::operation::IppOperation;
use ipp::prelude::*;

pub enum PrintError {
    /// LPrint unreachable or busy, try again later
    Retry(anyhow::Error),
    /// The printer rejected the job, retrying won't help
    Rejected(anyhow::Error),
}

pub struct Printer {
    uri: Uri,
    client: IppClient,
}

impl Printer {
    pub fn new(uri: &str) -> Result<Printer> {
        let uri: Uri = uri
            .parse()
            .with_context(|| format!("invalid PRINTER_URI {uri:?}"))?;
        let client = IppClient::builder(uri.clone())
            .request_timeout(Duration::from_secs(60))
            .build();
        Ok(Printer { uri, client })
    }

    /// The printer's resolution in dots per inch (180 or 203 for ESC/POS)
    pub fn resolution(&self) -> Result<u32, PrintError> {
        let op = IppOperationBuilder::get_printer_attributes(self.uri.clone())
            .attribute("printer-resolution-default")
            .build()
            .map_err(|e| PrintError::Rejected(e.into()))?;
        let response = self.send(op)?;
        let value = response
            .attributes()
            .groups_of(DelimiterTag::PrinterAttributes)
            .find_map(|g| g.get("printer-resolution-default"))
            .map(|a| a.value());
        match value {
            // units: 3 = dots per inch, 4 = dots per centimeter
            Some(&IppValue::Resolution {
                cross_feed,
                units: 3,
                ..
            }) if cross_feed > 0 => Ok(cross_feed as u32),
            Some(&IppValue::Resolution {
                cross_feed,
                units: 4,
                ..
            }) if cross_feed > 0 => Ok((f64::from(cross_feed) * 2.54).round() as u32),
            _ => Err(PrintError::Rejected(anyhow!(
                "printer reports no resolution"
            ))),
        }
    }

    /// Prints a PNG pixel for pixel on 80 mm roll media cut to the image length
    pub fn print(&self, title: &str, png: Vec<u8>, height_hmm: i32) -> Result<i32, PrintError> {
        let attr = |name: &str, value| {
            IppAttribute::with_name(name, value).map_err(|e| PrintError::Rejected(e.into()))
        };
        let keyword =
            |value: &str| IppValue::new_keyword(value).map_err(|e| PrintError::Rejected(e.into()));

        let media_size = collection([
            ("x-dimension", IppValue::Integer(8000)),
            // LPrint's ESC/POS rolls start at 10 mm
            ("y-dimension", IppValue::Integer(height_hmm.max(1000))),
        ])?;
        // The Typst page has its own 4 mm margins
        let media_col = collection([
            ("media-size", media_size),
            ("media-top-margin", IppValue::Integer(0)),
            ("media-bottom-margin", IppValue::Integer(0)),
            ("media-left-margin", IppValue::Integer(0)),
            ("media-right-margin", IppValue::Integer(0)),
        ])?;

        let op =
            IppOperationBuilder::print_job(self.uri.clone(), IppPayload::new(Cursor::new(png)))
                .job_title(title)
                .user_name("mobix-app")
                .document_format("image/png")
                .attribute(attr("media-col", media_col)?)
                .attribute(attr("print-scaling", keyword("none")?)?)
                // Never rotate a short receipt to landscape
                .attribute(attr("orientation-requested", IppValue::Enum(3))?)
                .build()
                .map_err(|e| PrintError::Rejected(e.into()))?;
        let response = self.send(op)?;

        if response.header().status_code() != StatusCode::SuccessfulOk {
            // Most likely media-col was ignored and the job prints on the default
            // (1 m) receipt length; LPrint has the details in its log
            log::warn!(
                "print job accepted with status {}",
                response.header().status_code()
            );
        }
        let job_id = response
            .attributes()
            .groups_of(DelimiterTag::JobAttributes)
            .find_map(|g| g.get("job-id"))
            .and_then(|a| match a.value() {
                IppValue::Integer(id) => Some(*id),
                _ => None,
            })
            .unwrap_or(0);
        Ok(job_id)
    }

    fn send(&self, op: impl IppOperation) -> Result<IppRequestResponse, PrintError> {
        let response = self
            .client
            .send(op)
            .map_err(|e| PrintError::Retry(anyhow!("cannot reach {}: {e}", self.uri)))?;
        let status = response.header().status_code();
        if status.is_success() {
            Ok(response)
        } else if (status as u16) >= 0x0500 {
            Err(PrintError::Retry(anyhow!("printer error {status}")))
        } else {
            Err(PrintError::Rejected(anyhow!(
                "printer rejected the request: {status}"
            )))
        }
    }
}

fn collection<const N: usize>(members: [(&str, IppValue); N]) -> Result<IppValue, PrintError> {
    let mut map = BTreeMap::new();
    for (name, value) in members {
        let name = name
            .try_into()
            .map_err(|e: ipp::parser::IppParseError| PrintError::Rejected(e.into()))?;
        map.insert(name, value);
    }
    Ok(IppValue::Collection(map))
}
