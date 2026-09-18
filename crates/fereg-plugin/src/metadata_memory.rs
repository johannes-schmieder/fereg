//! Checked admission of decoder tables and metadata-only compiler frames.
//!
//! No row-sized state is created here. Original observation classification
//! remains in the preparation layer, after input construction is admitted.

use fereg_core::error::{ErrorCode, FeregError, Result};
use fereg_core::metadata::{AbsorbedTerm, ExpandedTerm, SourceEntry, SourceId, TermComponent};
use fereg_core::ols::{
    OlsClusterTermV1, OlsInputBinding, PreparedOlsCategoricalComponent, PreparedOlsCoefficient,
    PreparedOlsColumn,
};
use std::mem::size_of;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MetadataShape {
    /// Source, term, component, absorb, absorb-component, cluster,
    /// cluster-component and binding row counts, in wire order.
    pub rows: [usize; 8],
    /// Capacities of all live specification strings and registry descriptors.
    pub specification_bytes: u128,
    /// ABI argument owners remain borrowed until the estimation call returns.
    pub transport_parent_bytes: u128,
}

impl MetadataShape {
    pub(crate) fn peak_bytes(self) -> Result<u128> {
        let [sources, terms, components, effects, effect_components, clusters, cluster_components, bindings] =
            self.rows;
        let mut tables = 0;
        for (rows, width) in self.rows.into_iter().zip([2, 8, 13, 4, 4, 2, 3, 3]) {
            // Empty SPI tables use one sentinel row. Count every input table
            // concurrently, although cluster/binding reads follow decode.
            tables = add(
                tables,
                product(rows.max(1), size_of::<Vec<f64>>() + 8 * width)?,
            )?;
        }
        let mut decoded = 0;
        for (count, width) in [
            (sources, size_of::<SourceEntry>()),
            (terms, size_of::<ExpandedTerm>()),
            (components, size_of::<TermComponent>()),
            (effects, size_of::<AbsorbedTerm>()),
            (effect_components, size_of::<SourceId>()),
            (clusters, size_of::<OlsClusterTermV1>()),
            (cluster_components, size_of::<SourceId>()),
            (bindings, size_of::<OlsInputBinding>()),
        ] {
            decoded = add(decoded, product(count, width)?)?;
        }
        let slots = terms.checked_add(1).ok_or_else(overflow)?;
        let compiled = add(
            product(
                slots,
                size_of::<PreparedOlsColumn>() + size_of::<PreparedOlsCoefficient>(),
            )?,
            product(components, size_of::<PreparedOlsCategoricalComponent>())?,
        )?;
        // Decoder headers and source-availability identities are additional
        // to the original/clustered/availability request copies. Their sizes
        // depend on wire counts, never on a Cartesian product or N.
        let descriptors = self
            .rows
            .into_iter()
            .try_fold(1_u128, |sum, value| add(sum, value as u128))?;
        // Metadata validation uses integer/reference BTree registries. A node
        // has at most eleven keys and twelve links; 1024 bytes bounds the
        // largest key/value node here. Two nodes per inserted key bounds a
        // split alongside the retained tree. Four complete registries cover
        // the simultaneous source/binding/effect validation frames.
        let registries_and_headers = descriptors
            .checked_mul(4 * 2 * 1024 + 256)
            .ok_or_else(overflow)?;
        let copies = decoded.checked_mul(4).ok_or_else(overflow)?;
        let strings = self
            .specification_bytes
            .checked_mul(8)
            .ok_or_else(overflow)?;
        add(
            self.transport_parent_bytes,
            add(
                add(tables, copies)?,
                add(add(compiled, registries_and_headers)?, strings)?,
            )?,
        )
    }

    pub(crate) fn admit(self, usable: Option<u128>) -> Result<u128> {
        let peak = self.peak_bytes()?;
        admit(peak, usable)?;
        Ok(peak)
    }
}

pub(crate) fn admit(peak: u128, usable: Option<u128>) -> Result<()> {
    if usable.is_some_and(|usable| peak > usable) {
        Err(FeregError::new(
            ErrorCode::Allocation,
            "metadata construction exceeds usable memory",
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn argument_parent_bytes(args: &[String]) -> Result<u128> {
    // Production argv is reserved exactly. Include its omitted action token
    // and descriptor as well as the borrowed tail; direct Rust callers receive
    // the same conservative allowance. Foreign caller storage is not native.
    let count = args.len().checked_add(1).ok_or_else(overflow)?;
    args.iter().try_fold(
        add(product(count, size_of::<String>())?, 32)?,
        |sum, value| add(sum, value.capacity() as u128),
    )
}

pub(crate) fn transport_peak_bytes(args: &[String]) -> Result<u128> {
    let payload = args
        .iter()
        .try_fold(0_u128, |sum, value| add(sum, value.capacity() as u128))?;
    // Framed fields, parsed values and specification registries coexist.
    // A registry entry consumes at least two input bytes ("0:"); its String
    // descriptor costs 24 bytes. Registry decoding retains no split vector.
    // 32 payload bytes cover those descriptors, all transport/value copies
    // and a worst-case escaped error string with geometric formatting growth.
    // Protocols accept at most 128 fields; two 1024-byte nodes per field
    // bound BTree growth independently of payload length.
    add(
        payload.checked_mul(32).ok_or_else(overflow)?,
        add(argument_parent_bytes(args)?, 128 * 2 * 1024)?,
    )
}

#[allow(clippy::ptr_arg)]
pub(crate) fn strings_bytes(values: &Vec<String>) -> u128 {
    values.capacity() as u128 * size_of::<String>() as u128
        + values
            .iter()
            .map(|value| value.capacity() as u128)
            .sum::<u128>()
}

fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or_else(overflow)
}
fn product(a: usize, b: usize) -> Result<u128> {
    (a as u128).checked_mul(b as u128).ok_or_else(overflow)
}
fn overflow() -> FeregError {
    FeregError::new(
        ErrorCode::Allocation,
        "metadata construction size overflows",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_boundary_is_exact_and_checked_before_construction() {
        for rows in [[0; 8], [3, 5, 8, 2, 4, 1, 2, 5], [100_000; 8]] {
            let shape = MetadataShape {
                rows,
                specification_bytes: 1234,
                transport_parent_bytes: 197,
            };
            let bytes = shape.peak_bytes().unwrap();
            assert_eq!(shape.admit(Some(bytes)).unwrap(), bytes);
            assert_eq!(
                shape.admit(Some(bytes - 1)).unwrap_err().code,
                ErrorCode::Allocation
            );
        }
        assert!(MetadataShape {
            rows: [usize::MAX; 8],
            specification_bytes: u128::MAX,
            transport_parent_bytes: 1,
        }
        .peak_bytes()
        .is_err());
    }
}
