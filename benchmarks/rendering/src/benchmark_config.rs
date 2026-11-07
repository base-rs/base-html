// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use serde::Serialize;
use std::io;
use std::str::FromStr;

/// A workload boundary; comparisons are meaningful only within the same workload and parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Workload {
    Static,
    List,
    Escape,
    Http,
}

/// The macro/core engine path or its independently written equivalent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    Engine,
    Reference,
}

/// Deterministic source text with a specified UTF-8 byte length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Corpus {
    Plain,
    Escaped,
    Unicode,
}

macro_rules! parse_enum {
    ($ty:ty, $($text:literal => $variant:path),+ $(,)?) => {
        impl FromStr for $ty {
            type Err = io::Error;
            fn from_str(value: &str) -> io::Result<Self> {
                match value { $($text => Ok($variant),)+ _ => Err(invalid(format!("invalid {}: {value}", stringify!($ty)))) }
            }
        }
    };
}

parse_enum!(Workload, "static" => Workload::Static, "list" => Workload::List, "escape" => Workload::Escape, "http" => Workload::Http);
parse_enum!(Variant, "engine" => Variant::Engine, "reference" => Variant::Reference);
parse_enum!(Corpus, "plain" => Corpus::Plain, "escaped" => Corpus::Escaped, "unicode" => Corpus::Unicode);

/// All workload parameters, repeated in the output record for reproducibility.
#[derive(Clone, Debug, Serialize)]
pub struct BenchmarkConfig {
    pub workload: Workload,
    pub variant: Variant,
    pub corpus: Corpus,
    pub iterations: u64,
    pub warmup: u64,
    pub items: usize,
    pub text_bytes: usize,
    pub buffer_bytes: usize,
    pub consumer_yields: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            workload: Workload::List,
            variant: Variant::Engine,
            corpus: Corpus::Unicode,
            iterations: 10_000,
            warmup: 1_000,
            items: 32,
            text_bytes: 64,
            buffer_bytes: 8192,
            consumer_yields: 0,
        }
    }
}

impl BenchmarkConfig {
    /// Parse explicit `--name value` options. Unknown, duplicate, and incomplete options are errors.
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut config = Self::default();
        let mut arguments = arguments.into_iter();
        let mut seen = std::collections::HashSet::new();
        while let Some(name) = arguments.next() {
            if !seen.insert(name.clone()) {
                return Err(invalid(format!("duplicate option: {name}")));
            }
            let value = arguments
                .next()
                .ok_or_else(|| invalid(format!("missing value for {name}")))?;
            match name.as_str() {
                "--workload" => config.workload = value.parse()?,
                "--variant" => config.variant = value.parse()?,
                "--corpus" => config.corpus = value.parse()?,
                "--iterations" => config.iterations = number(&name, &value)?,
                "--warmup" => config.warmup = number(&name, &value)?,
                "--items" => config.items = number(&name, &value)?,
                "--text-bytes" => config.text_bytes = number(&name, &value)?,
                "--buffer-bytes" => config.buffer_bytes = number(&name, &value)?,
                "--consumer-yields" => config.consumer_yields = number(&name, &value)?,
                _ => return Err(invalid(format!("unknown option: {name}"))),
            }
        }
        config.validate()?;
        Ok(config)
    }

    /// Reject invalid sizes and meaningless combinations before allocating or measuring.
    // SECURITY: Sizes and counts have no upper limit, so huge values can exhaust memory or run indefinitely, sometimes aborting rather than returning an error. Arguments come only from the operator and the repository's scenario metadata, who can already run arbitrary code here, and every failure exits without a record.
    pub fn validate(&self) -> io::Result<()> {
        if self.iterations == 0 || self.buffer_bytes == 0 {
            return Err(invalid("iterations and buffer-bytes must be nonzero"));
        }
        if self.consumer_yields != 0 && self.workload != Workload::Http {
            return Err(invalid("consumer-yields applies only to the http workload"));
        }
        self.items
            .checked_mul(self.text_bytes)
            .ok_or_else(|| invalid("items * text-bytes overflows usize"))?;
        Ok(())
    }
}

fn number<T: FromStr>(name: &str, value: &str) -> io::Result<T> {
    value
        .parse()
        .map_err(|_| invalid(format!("invalid nonnegative integer for {name}: {value}")))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
