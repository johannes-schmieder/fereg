use crate::error::{ErrorCode, FeregError, Result};
use std::collections::BTreeMap;

pub const ADO_VERSION: &str = "0.5.0";
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MINIMUM_STATA_MAJOR: u16 = 18;
pub const SPI_MAJOR: u16 = 3;
pub const SPI_MINOR: u16 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

/// Version of the small action/diagnostics handshake accepted by the plugin.
///
/// Minor 5 adds the explicit ado-certified complete-input flag used by the
/// estimation bridge. Older diagnostic callers remain valid under the usual
/// minor-version rule; estimation continues to require an exact action wire.
pub const ACTION_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::new(1, 5);
/// Version of the data-independent expanded-model metadata wire.
pub const MODEL_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(2, 1);
/// Version of the public prepared-OLS shadow-result layout.
pub const OLS_RESULT_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 4);
/// Version of the PPML-specific request layout, independent of the action,
/// model, OLS-result, and PPML-result wires.
/// Minor 5 replaces the private solver-development cross-product with the
/// single public `solver(auto|map|lsmr|cmg)` family field.
pub const PPML_REQUEST_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 5);
/// Version of the separately versioned PPML shadow-result layout.
/// Minor 2 adds the fixed-shape public PPML solver report.
pub const PPML_RESULT_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 2);

/// Exact integer magic stored in the first column of every PPML result receipt.
///
/// This value is deliberately distinct from the OLS result magic. Both are
/// exactly representable in a Stata/Rust double, so native validation never
/// depends on floating-point tolerance.
pub const PPML_RESULT_MAGIC: f64 = 5_354_602.0;
/// Fixed row counts in the mandatory PPML 1.2 evidence matrices.
pub const PPML_RESULT_INFERENCE_ROWS: usize = 16;
pub const PPML_RESULT_DROP_COUNT_ROWS: usize = 14;
pub const PPML_RESULT_SEPARATION_SEQUENCE_ROWS: usize = 5;
/// Number of columns in the PPML 1.2 result receipt.
pub const PPML_RESULT_RECEIPT_COLUMNS: usize = 33;
/// One-based column containing the receipt-last transaction sentinel.
pub const PPML_RESULT_RECEIPT_COMPLETE_COLUMN: usize = PPML_RESULT_RECEIPT_COLUMNS;
/// Value stored in the final receipt column only after every other output has
/// been validated and published successfully.
pub const PPML_RESULT_RECEIPT_COMPLETE_VALUE: f64 = 1.0;

const MAX_EXACT_F64_INTEGER: u128 = 9_007_199_254_740_992;

/// One-based PPML result-receipt columns for wire version 1.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PpmlResultReceiptFieldV1 {
    Magic = 1,
    Major = 2,
    Minor = 3,
    Vce = 4,
    CoefficientCount = 5,
    CovarianceDimension = 6,
    OriginalObservations = 7,
    CandidateObservations = 8,
    FinalObservations = 9,
    ClusterDimensions = 10,
    FixedEffectRows = 11,
    ClusterSubsetRows = 12,
    SamplePresent = 13,
    ResidualPresent = 14,
    MandatoryCertificatesPassed = 15,
    SeparatedObservations = 16,
    SeparationReportRows = 17,
    SavedFixedEffectChannels = 18,
    Adjustment = 19,
    InferenceRows = 20,
    DropCountRows = 21,
    SeparationSequenceRows = 22,
    SeparationExecutionCount = 23,
    SeparationGenerationCount = 24,
    InitialGenerationHashHigh = 25,
    InitialGenerationHashLow = 26,
    FinalGenerationHashHigh = 27,
    FinalGenerationHashLow = 28,
    SeparationTraceDigestHigh = 29,
    SeparationTraceDigestLow = 30,
    GenerationHashDigestHigh = 31,
    GenerationHashDigestLow = 32,
    Complete = 33,
}

impl PpmlResultReceiptFieldV1 {
    pub const ALL: [Self; PPML_RESULT_RECEIPT_COLUMNS] = [
        Self::Magic,
        Self::Major,
        Self::Minor,
        Self::Vce,
        Self::CoefficientCount,
        Self::CovarianceDimension,
        Self::OriginalObservations,
        Self::CandidateObservations,
        Self::FinalObservations,
        Self::ClusterDimensions,
        Self::FixedEffectRows,
        Self::ClusterSubsetRows,
        Self::SamplePresent,
        Self::ResidualPresent,
        Self::MandatoryCertificatesPassed,
        Self::SeparatedObservations,
        Self::SeparationReportRows,
        Self::SavedFixedEffectChannels,
        Self::Adjustment,
        Self::InferenceRows,
        Self::DropCountRows,
        Self::SeparationSequenceRows,
        Self::SeparationExecutionCount,
        Self::SeparationGenerationCount,
        Self::InitialGenerationHashHigh,
        Self::InitialGenerationHashLow,
        Self::FinalGenerationHashHigh,
        Self::FinalGenerationHashLow,
        Self::SeparationTraceDigestHigh,
        Self::SeparationTraceDigestLow,
        Self::GenerationHashDigestHigh,
        Self::GenerationHashDigestLow,
        Self::Complete,
    ];

    /// One-based matrix column used by the Stata result wire.
    #[must_use]
    pub const fn column(self) -> usize {
        self as usize
    }

    const fn index(self) -> usize {
        self.column() - 1
    }
}

/// Stable PPML VCE codes used only by the PPML result wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PpmlResultVceV1 {
    ModelBased = 1,
    Robust = 2,
    Clustered = 3,
}

impl PpmlResultVceV1 {
    #[must_use]
    pub const fn code(self) -> f64 {
        self as u16 as f64
    }
}

/// Stable offset/exposure codes used only by the PPML result wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PpmlAdjustmentV1 {
    None = 0,
    Offset = 1,
    Exposure = 2,
}

impl PpmlAdjustmentV1 {
    #[must_use]
    pub const fn code(self) -> f64 {
        self as u16 as f64
    }
}

/// Structural PPML receipt data before transactional publication.
///
/// `encode_incomplete` intentionally has no `complete` argument: the final
/// sentinel belongs to the output transaction, not to the scientific result.
/// The plugin must store `PPML_RESULT_RECEIPT_COMPLETE_VALUE` in
/// `PPML_RESULT_RECEIPT_COMPLETE_COLUMN` only after all other stores succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmlResultReceiptV1 {
    pub vce: PpmlResultVceV1,
    pub coefficient_count: usize,
    pub covariance_dimension: usize,
    pub original_observations: usize,
    pub candidate_observations: usize,
    pub final_observations: usize,
    pub cluster_dimensions: usize,
    pub fixed_effect_rows: usize,
    pub cluster_subset_rows: usize,
    pub sample_present: bool,
    pub residual_present: bool,
    pub mandatory_certificates_passed: bool,
    pub separated_observations: usize,
    pub separation_report_rows: usize,
    pub saved_fixed_effect_channels: usize,
    pub adjustment: PpmlAdjustmentV1,
    pub inference_rows: usize,
    pub drop_count_rows: usize,
    pub separation_sequence_rows: usize,
    pub separation_execution_count: usize,
    pub separation_generation_count: usize,
    pub initial_generation_hash: u64,
    pub final_generation_hash: u64,
    pub separation_trace_digest: u64,
    pub generation_hash_digest: u64,
}

impl PpmlResultReceiptV1 {
    /// Encode a structurally validated receipt with the COMPLETE sentinel zero.
    pub fn encode_incomplete(self) -> Result<[f64; PPML_RESULT_RECEIPT_COLUMNS]> {
        self.validate()?;
        let mut receipt = [0.0_f64; PPML_RESULT_RECEIPT_COLUMNS];
        receipt[PpmlResultReceiptFieldV1::Magic.index()] = PPML_RESULT_MAGIC;
        receipt[PpmlResultReceiptFieldV1::Major.index()] =
            f64::from(PPML_RESULT_WIRE_VERSION.major);
        receipt[PpmlResultReceiptFieldV1::Minor.index()] =
            f64::from(PPML_RESULT_WIRE_VERSION.minor);
        receipt[PpmlResultReceiptFieldV1::Vce.index()] = self.vce.code();
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::CoefficientCount,
            self.coefficient_count,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::CovarianceDimension,
            self.covariance_dimension,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::OriginalObservations,
            self.original_observations,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::CandidateObservations,
            self.candidate_observations,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::FinalObservations,
            self.final_observations,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::ClusterDimensions,
            self.cluster_dimensions,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::FixedEffectRows,
            self.fixed_effect_rows,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::ClusterSubsetRows,
            self.cluster_subset_rows,
        );
        receipt[PpmlResultReceiptFieldV1::SamplePresent.index()] = f64::from(self.sample_present);
        receipt[PpmlResultReceiptFieldV1::ResidualPresent.index()] =
            f64::from(self.residual_present);
        receipt[PpmlResultReceiptFieldV1::MandatoryCertificatesPassed.index()] =
            f64::from(self.mandatory_certificates_passed);
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::SeparatedObservations,
            self.separated_observations,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::SeparationReportRows,
            self.separation_report_rows,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::SavedFixedEffectChannels,
            self.saved_fixed_effect_channels,
        );
        receipt[PpmlResultReceiptFieldV1::Adjustment.index()] = self.adjustment.code();
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::InferenceRows,
            self.inference_rows,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::DropCountRows,
            self.drop_count_rows,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::SeparationSequenceRows,
            self.separation_sequence_rows,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::SeparationExecutionCount,
            self.separation_execution_count,
        );
        set_receipt_count(
            &mut receipt,
            PpmlResultReceiptFieldV1::SeparationGenerationCount,
            self.separation_generation_count,
        );
        set_receipt_hash(
            &mut receipt,
            PpmlResultReceiptFieldV1::InitialGenerationHashHigh,
            PpmlResultReceiptFieldV1::InitialGenerationHashLow,
            self.initial_generation_hash,
        );
        set_receipt_hash(
            &mut receipt,
            PpmlResultReceiptFieldV1::FinalGenerationHashHigh,
            PpmlResultReceiptFieldV1::FinalGenerationHashLow,
            self.final_generation_hash,
        );
        set_receipt_hash(
            &mut receipt,
            PpmlResultReceiptFieldV1::SeparationTraceDigestHigh,
            PpmlResultReceiptFieldV1::SeparationTraceDigestLow,
            self.separation_trace_digest,
        );
        set_receipt_hash(
            &mut receipt,
            PpmlResultReceiptFieldV1::GenerationHashDigestHigh,
            PpmlResultReceiptFieldV1::GenerationHashDigestLow,
            self.generation_hash_digest,
        );
        debug_assert_eq!(receipt[PpmlResultReceiptFieldV1::Complete.index()], 0.0);
        Ok(receipt)
    }

    fn validate(self) -> Result<()> {
        let counts = [
            self.coefficient_count,
            self.covariance_dimension,
            self.original_observations,
            self.candidate_observations,
            self.final_observations,
            self.cluster_dimensions,
            self.fixed_effect_rows,
            self.cluster_subset_rows,
            self.separated_observations,
            self.separation_report_rows,
            self.saved_fixed_effect_channels,
            self.inference_rows,
            self.drop_count_rows,
            self.separation_sequence_rows,
            self.separation_execution_count,
            self.separation_generation_count,
        ];
        if counts
            .iter()
            .any(|value| *value as u128 > MAX_EXACT_F64_INTEGER)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML result receipt count is not exactly representable as f64",
            ));
        }
        if self.covariance_dimension > self.coefficient_count {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML covariance dimension exceeds the logical coefficient count",
            ));
        }
        if self.candidate_observations > self.original_observations
            || self.final_observations > self.candidate_observations
            || self.separated_observations
                > self
                    .candidate_observations
                    .saturating_sub(self.final_observations)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML result receipt sample counts are inconsistent",
            ));
        }
        if !self.sample_present || !self.mandatory_certificates_passed {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML successful result receipt lacks a mandatory output or certificate",
            ));
        }
        if self.inference_rows != PPML_RESULT_INFERENCE_ROWS
            || self.drop_count_rows != PPML_RESULT_DROP_COUNT_ROWS
            || self.separation_sequence_rows != PPML_RESULT_SEPARATION_SEQUENCE_ROWS
            || self.separation_generation_count >= self.candidate_observations
            || self.separation_execution_count
                > self
                    .candidate_observations
                    .saturating_mul(PPML_RESULT_SEPARATION_SEQUENCE_ROWS)
        {
            return Err(FeregError::new(
                ErrorCode::InvalidArgument,
                "PPML result receipt has inconsistent bounded evidence metadata",
            ));
        }
        match self.vce {
            PpmlResultVceV1::Clustered => {
                let shift = u32::try_from(self.cluster_dimensions).map_err(|_| {
                    FeregError::new(
                        ErrorCode::InvalidArgument,
                        "PPML cluster dimension count overflows the receipt layout",
                    )
                })?;
                let expected = 1_usize
                    .checked_shl(shift)
                    .and_then(|subsets| subsets.checked_sub(1))
                    .ok_or_else(|| {
                        FeregError::new(
                            ErrorCode::InvalidArgument,
                            "PPML cluster dimension count overflows the receipt layout",
                        )
                    })?;
                if self.cluster_dimensions == 0 || self.cluster_subset_rows != expected {
                    return Err(FeregError::new(
                        ErrorCode::InvalidArgument,
                        "clustered PPML receipt has inconsistent cluster dimensions or subsets",
                    ));
                }
            }
            PpmlResultVceV1::ModelBased | PpmlResultVceV1::Robust => {
                if self.cluster_dimensions != 0 || self.cluster_subset_rows != 0 {
                    return Err(FeregError::new(
                        ErrorCode::InvalidArgument,
                        "nonclustered PPML receipt contains cluster metadata",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn set_receipt_count(
    receipt: &mut [f64; PPML_RESULT_RECEIPT_COLUMNS],
    field: PpmlResultReceiptFieldV1,
    value: usize,
) {
    receipt[field.index()] = value as f64;
}

fn set_receipt_hash(
    receipt: &mut [f64; PPML_RESULT_RECEIPT_COLUMNS],
    high_field: PpmlResultReceiptFieldV1,
    low_field: PpmlResultReceiptFieldV1,
    value: u64,
) {
    receipt[high_field.index()] = f64::from((value >> 32) as u32);
    receipt[low_field.index()] = f64::from(value as u32);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub ado_version: String,
    pub protocol: ProtocolVersion,
    pub stata_major: u16,
}

impl Handshake {
    #[must_use]
    pub fn current(stata_major: u16) -> Self {
        Self {
            ado_version: ADO_VERSION.to_owned(),
            protocol: ACTION_PROTOCOL_VERSION,
            stata_major,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.protocol.major != ACTION_PROTOCOL_VERSION.major {
            return Err(FeregError::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "action protocol major {} is incompatible with plugin major {}",
                    self.protocol.major, ACTION_PROTOCOL_VERSION.major
                ),
            ));
        }
        if self.protocol.minor > ACTION_PROTOCOL_VERSION.minor {
            return Err(FeregError::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "action protocol minor {} is newer than plugin minor {}",
                    self.protocol.minor, ACTION_PROTOCOL_VERSION.minor
                ),
            ));
        }
        if self.stata_major < MINIMUM_STATA_MAJOR {
            return Err(FeregError::new(
                ErrorCode::Unsupported,
                format!(
                    "Stata {} is unsupported; fereg requires Stata {} or newer",
                    self.stata_major, MINIMUM_STATA_MAJOR
                ),
            ));
        }
        if self.ado_version != ADO_VERSION {
            return Err(FeregError::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "ado contract {} is incompatible with plugin contract {}",
                    self.ado_version, ADO_VERSION
                ),
            ));
        }
        Ok(())
    }

    pub fn from_key_values(values: &BTreeMap<String, String>) -> Result<Self> {
        let ado_version = required(values, "ado")?.to_owned();
        let protocol_major = parse_u16(values, "protocol_major")?;
        let protocol_minor = parse_u16(values, "protocol_minor")?;
        let stata_major = parse_stata_major(required(values, "stata")?)?;
        Ok(Self {
            ado_version,
            protocol: ProtocolVersion::new(protocol_major, protocol_minor),
            stata_major,
        })
    }
}

pub fn parse_key_values(args: &[String]) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for argument in args {
        let (key, value) = argument.split_once('=').ok_or_else(|| {
            FeregError::new(
                ErrorCode::Syntax,
                format!("plugin argument {argument:?} must have key=value form"),
            )
        })?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if key.is_empty() || value.is_empty() {
            return Err(FeregError::new(
                ErrorCode::Syntax,
                format!("plugin argument {argument:?} has an empty key or value"),
            ));
        }
        if values.insert(key.clone(), value).is_some() {
            return Err(FeregError::new(
                ErrorCode::Syntax,
                format!("duplicate plugin protocol field {key}"),
            ));
        }
    }
    Ok(values)
}

fn required<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str> {
    values.get(key).map(String::as_str).ok_or_else(|| {
        FeregError::new(
            ErrorCode::Syntax,
            format!("missing required protocol field {key}=..."),
        )
    })
}

fn parse_u16(values: &BTreeMap<String, String>, key: &str) -> Result<u16> {
    required(values, key)?.parse::<u16>().map_err(|_| {
        FeregError::new(
            ErrorCode::Syntax,
            format!("protocol field {key} must be an unsigned integer"),
        )
    })
}

fn parse_stata_major(value: &str) -> Result<u16> {
    value
        .split('.')
        .next()
        .unwrap_or(value)
        .parse::<u16>()
        .map_err(|_| {
            FeregError::new(
                ErrorCode::Syntax,
                format!("invalid Stata version {value:?}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_handshake_passes() {
        Handshake::current(18)
            .validate()
            .expect("current handshake");
    }

    #[test]
    fn action_handshake_and_model_wire_are_distinct_versions() {
        assert_eq!(ACTION_PROTOCOL_VERSION, ProtocolVersion::new(1, 5));
        assert_eq!(MODEL_WIRE_VERSION, ProtocolVersion::new(2, 1));
        assert_eq!(OLS_RESULT_WIRE_VERSION, ProtocolVersion::new(1, 4));
        assert_eq!(PPML_REQUEST_WIRE_VERSION, ProtocolVersion::new(1, 5));
        assert_eq!(PPML_RESULT_WIRE_VERSION, ProtocolVersion::new(1, 2));
        assert_ne!(ACTION_PROTOCOL_VERSION, MODEL_WIRE_VERSION);
        assert_ne!(OLS_RESULT_WIRE_VERSION, PPML_RESULT_WIRE_VERSION);
    }

    #[test]
    fn ppml_receipt_layout_is_closed_and_complete_is_last() {
        let columns = PpmlResultReceiptFieldV1::ALL.map(PpmlResultReceiptFieldV1::column);
        assert_eq!(columns, std::array::from_fn(|index| index + 1));
        assert_eq!(PPML_RESULT_RECEIPT_COLUMNS, 33);
        assert_eq!(PPML_RESULT_RECEIPT_COMPLETE_COLUMN, 33);
        assert_eq!(PpmlResultReceiptFieldV1::Complete.column(), 33);
        assert_eq!(PPML_RESULT_RECEIPT_COMPLETE_VALUE, 1.0);
    }

    #[test]
    fn ppml_receipt_encodes_exact_identity_and_remains_incomplete() {
        let receipt = valid_ppml_receipt()
            .encode_incomplete()
            .expect("valid receipt");
        assert_eq!(
            receipt,
            [
                5_354_602.0,
                1.0,
                1.0,
                3.0,
                5.0,
                4.0,
                100.0,
                90.0,
                80.0,
                2.0,
                3.0,
                3.0,
                1.0,
                1.0,
                1.0,
                7.0,
                3.0,
                2.0,
                2.0,
                16.0,
                14.0,
                5.0,
                4.0,
                2.0,
                2_877_908_810.0,
                2_271_560_481.0,
                2_307_356_243.0,
                427_565_583.0,
                287_454_020.0,
                1_432_778_632.0,
                2_886_935_731.0,
                1_717_991_265.0,
                0.0,
            ]
        );
        assert_eq!(receipt[PPML_RESULT_RECEIPT_COMPLETE_COLUMN - 1], 0.0);
        assert_ne!(PPML_RESULT_MAGIC, 5_354_601.0);
    }

    #[test]
    fn ppml_receipt_rejects_inconsistent_structure() {
        let mut receipt = valid_ppml_receipt();
        receipt.covariance_dimension = receipt.coefficient_count + 1;
        assert_eq!(
            receipt
                .encode_incomplete()
                .expect_err("covariance bound")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut receipt = valid_ppml_receipt();
        receipt.cluster_subset_rows = 2;
        assert_eq!(
            receipt
                .encode_incomplete()
                .expect_err("subset inventory")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut receipt = valid_ppml_receipt();
        receipt.separated_observations = 11;
        assert_eq!(
            receipt
                .encode_incomplete()
                .expect_err("separation bound")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut receipt = valid_ppml_receipt();
        receipt.mandatory_certificates_passed = false;
        assert_eq!(
            receipt.encode_incomplete().expect_err("certificates").code,
            ErrorCode::InvalidArgument
        );

        let mut receipt = valid_ppml_receipt();
        receipt.inference_rows -= 1;
        assert_eq!(
            receipt
                .encode_incomplete()
                .expect_err("mandatory evidence dimensions")
                .code,
            ErrorCode::InvalidArgument
        );

        let mut receipt = valid_ppml_receipt();
        receipt.separation_execution_count =
            receipt.candidate_observations * PPML_RESULT_SEPARATION_SEQUENCE_ROWS + 1;
        assert_eq!(
            receipt
                .encode_incomplete()
                .expect_err("bounded execution count")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    fn valid_ppml_receipt() -> PpmlResultReceiptV1 {
        PpmlResultReceiptV1 {
            vce: PpmlResultVceV1::Clustered,
            coefficient_count: 5,
            covariance_dimension: 4,
            original_observations: 100,
            candidate_observations: 90,
            final_observations: 80,
            cluster_dimensions: 2,
            fixed_effect_rows: 3,
            cluster_subset_rows: 3,
            sample_present: true,
            residual_present: true,
            mandatory_certificates_passed: true,
            separated_observations: 7,
            separation_report_rows: 3,
            saved_fixed_effect_channels: 2,
            adjustment: PpmlAdjustmentV1::Exposure,
            inference_rows: PPML_RESULT_INFERENCE_ROWS,
            drop_count_rows: PPML_RESULT_DROP_COUNT_ROWS,
            separation_sequence_rows: PPML_RESULT_SEPARATION_SEQUENCE_ROWS,
            separation_execution_count: 4,
            separation_generation_count: 2,
            initial_generation_hash: 0xab89_674a_8765_4321,
            final_generation_hash: 0x8987_7653_197c_220f,
            separation_trace_digest: 0x1122_3344_5566_7788,
            generation_hash_digest: 0xac13_24b3_6666_7761,
        }
    }

    #[test]
    fn newer_protocol_major_fails_closed() {
        let handshake = Handshake {
            ado_version: ADO_VERSION.to_owned(),
            protocol: ProtocolVersion::new(ACTION_PROTOCOL_VERSION.major + 1, 0),
            stata_major: 18,
        };
        let error = handshake.validate().expect_err("must reject");
        assert_eq!(error.code, ErrorCode::ProtocolMismatch);
        assert!(error.message.contains("action protocol major"));
        assert!(!error.message.contains("model protocol"));
    }

    #[test]
    fn newer_action_protocol_minor_is_labeled_unambiguously() {
        let handshake = Handshake {
            ado_version: ADO_VERSION.to_owned(),
            protocol: ProtocolVersion::new(
                ACTION_PROTOCOL_VERSION.major,
                ACTION_PROTOCOL_VERSION.minor + 1,
            ),
            stata_major: 18,
        };
        let error = handshake.validate().expect_err("must reject");
        assert_eq!(error.code, ErrorCode::ProtocolMismatch);
        assert!(error.message.contains("action protocol minor"));
        assert!(!error.message.contains("model protocol"));
    }

    #[test]
    fn old_stata_is_rejected() {
        assert_eq!(
            Handshake::current(17)
                .validate()
                .expect_err("must reject")
                .code,
            ErrorCode::Unsupported
        );
    }

    #[test]
    fn pre_one_ado_contract_must_match_exactly() {
        let handshake = Handshake {
            ado_version: "0.99.0".to_owned(),
            protocol: ACTION_PROTOCOL_VERSION,
            stata_major: 18,
        };
        assert_eq!(
            handshake.validate().expect_err("must reject").code,
            ErrorCode::ProtocolMismatch
        );
    }

    #[test]
    fn malformed_and_duplicate_key_values_are_rejected() {
        assert_eq!(
            parse_key_values(&["threads".to_owned()])
                .expect_err("must reject")
                .code,
            ErrorCode::Syntax
        );
        assert_eq!(
            parse_key_values(&["threads=1".to_owned(), "threads=2".to_owned()])
                .expect_err("must reject")
                .code,
            ErrorCode::Syntax
        );
    }
}
