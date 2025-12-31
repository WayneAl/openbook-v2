use anchor_lang::prelude::*;

#[account(zero_copy(unsafe))]
#[repr(packed)]
#[derive(PartialEq)]
pub struct AggregatorAccountData {
    /// Name of the aggregator to store on-chain.
    pub name: [u8; 32],
    /// Metadata of the aggregator to store on-chain.
    pub metadata: [u8; 128],
    /// Reserved.
    pub _reserved1: [u8; 32],
    /// Pubkey of the queue the aggregator belongs to.
    pub queue_pubkey: Pubkey,
    /// CONFIGS
    /// Number of oracles assigned to an update request.
    pub oracle_request_batch_size: u32,
    /// Minimum number of oracle responses required before a round is validated.
    pub min_oracle_results: u32,
    /// Minimum number of job results before an oracle accepts a result.
    pub min_job_results: u32,
    /// Minimum number of seconds required between aggregator rounds.
    pub min_update_delay_seconds: u32,
    /// Unix timestamp for which no feed update will occur before.
    pub start_after: i64,
    /// Change percentage required between a previous round and the current round. If variance percentage is not met, reject new oracle responses.
    pub variance_threshold: SwitchboardDecimal,
    /// Number of seconds for which, even if the variance threshold is not passed, accept new responses from oracles.
    pub force_report_period: i64,
    /// Timestamp when the feed is no longer needed.
    pub expiration: i64,
    //
    /// Counter for the number of consecutive failures before a feed is removed from a queue. If set to 0, failed feeds will remain on the queue.
    pub consecutive_failure_count: u64,
    /// Timestamp when the next update request will be available.
    pub next_allowed_update_time: i64,
    /// Flag for whether an aggregators configuration is locked for editing.
    pub is_locked: bool,
    /// Optional, public key of the crank the aggregator is currently using. Event based feeds do not need a crank.
    pub crank_pubkey: Pubkey,
    /// Latest confirmed update request result that has been accepted as valid.
    pub latest_confirmed_round: AggregatorRound,
    /// Oracle results from the current round of update request that has not been accepted as valid yet.
    pub current_round: AggregatorRound,
    /// List of public keys containing the job definitions for how data is sourced off-chain by oracles.
    pub job_pubkeys_data: [Pubkey; 16],
    /// Used to protect against malicious RPC nodes providing incorrect task definitions to oracles before fulfillment.
    pub job_hashes: [Hash; 16],
    /// Number of jobs assigned to an oracle.
    pub job_pubkeys_size: u32,
    /// Used to protect against malicious RPC nodes providing incorrect task definitions to oracles before fulfillment.
    pub jobs_checksum: [u8; 32],
    //
    /// The account delegated as the authority for making account changes.
    pub authority: Pubkey,
    /// Optional, public key of a history buffer account storing the last N accepted results and their timestamps.
    pub history_buffer: Pubkey,
    /// The previous confirmed round result.
    pub previous_confirmed_round_result: SwitchboardDecimal,
    /// The slot when the previous confirmed round was opened.
    pub previous_confirmed_round_slot: u64,
    /// 	Whether an aggregator is permitted to join a crank.
    pub disable_crank: bool,
    /// Job weights used for the weighted median of the aggregator's assigned job accounts.
    pub job_weights: [u8; 16],
    /// Unix timestamp when the feed was created.
    pub creation_timestamp: i64,
    /// Use sliding windoe or round based resolution
    /// NOTE: This changes result propogation in latest_round_result
    pub resolution_mode: AggregatorResolutionMode,
    /// Reserved for future info.
    pub _ebuf: [u8; 138],
}
impl Default for AggregatorAccountData {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[derive(Copy, Clone, Debug, AnchorSerialize, AnchorDeserialize, Eq, PartialEq)]
#[repr(u8)]
pub enum AggregatorResolutionMode {
    ModeRoundResolution = 0,
    ModeSlidingResolution = 1,
}

#[zero_copy(unsafe)]
#[repr(packed)]
#[derive(Default, Debug, Eq, PartialEq, AnchorDeserialize)]
pub struct SwitchboardDecimal {
    /// The part of a floating-point number that represents the significant digits of that number, and that is multiplied by the base, 10, raised to the power of scale to give the actual value of the number.
    pub mantissa: i128,
    /// The number of decimal places to move to the left to yield the actual value.
    pub scale: u32,
}

#[zero_copy(unsafe)]
#[repr(packed)]
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Hash {
    /// The bytes used to derive the hash.
    pub data: [u8; 32],
}

#[zero_copy(unsafe)]
#[repr(packed)]
#[derive(Default, PartialEq, Eq)]
pub struct AggregatorRound {
    /// Maintains the number of successful responses received from nodes.
    /// Nodes can submit one successful response per round.
    pub num_success: u32,
    /// Number of error responses.
    pub num_error: u32,
    /// Whether an update request round has ended.
    pub is_closed: bool,
    /// Maintains the `solana_program::clock::Slot` that the round was opened at.
    pub round_open_slot: u64,
    /// Maintains the `solana_program::clock::UnixTimestamp;` the round was opened at.
    pub round_open_timestamp: i64,
    /// Maintains the current median of all successful round responses.
    pub result: SwitchboardDecimal,
    /// Standard deviation of the accepted results in the round.
    pub std_deviation: SwitchboardDecimal,
    /// Maintains the minimum node response this round.
    pub min_response: SwitchboardDecimal,
    /// Maintains the maximum node response this round.
    pub max_response: SwitchboardDecimal,
    /// Pubkeys of the oracles fulfilling this round.
    pub oracle_pubkeys_data: [Pubkey; 16],
    /// Represents all successful node responses this round. `NaN` if empty.
    pub medians_data: [SwitchboardDecimal; 16],
    /// Current rewards/slashes oracles have received this round.
    pub current_payout: [i64; 16],
    /// Keep track of which responses are fulfilled here.
    pub medians_fulfilled: [bool; 16],
    /// Keeps track of which errors are fulfilled here.
    pub errors_fulfilled: [bool; 16],
}

impl AggregatorAccountData {
    /// If sufficient oracle responses, returns the latest on-chain result in SwitchboardDecimal format
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use switchboard_solana::AggregatorAccountData;
    /// use std::convert::TryInto;
    ///
    /// let feed_result = AggregatorAccountData::new(feed_account_info)?.get_result()?;
    /// let decimal: f64 = feed_result.try_into()?;
    /// ```
    pub fn get_result(&self) -> anchor_lang::Result<SwitchboardDecimal> {
        if self.resolution_mode == AggregatorResolutionMode::ModeSlidingResolution {
            return Ok(self.latest_confirmed_round.result);
        }
        let min_oracle_results = self.min_oracle_results;
        let latest_confirmed_round_num_success = self.latest_confirmed_round.num_success;
        if min_oracle_results > latest_confirmed_round_num_success {
            return Err(SwitchboardError::InvalidAggregatorRound.into());
        }
        Ok(self.latest_confirmed_round.result)
    }
}

use core::cmp::Ordering;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use std::convert::{From, TryInto};

#[derive(Default, Eq, PartialEq, Copy, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct BorshDecimal {
    pub mantissa: i128,
    pub scale: u32,
}
impl From<Decimal> for BorshDecimal {
    fn from(s: Decimal) -> Self {
        Self {
            mantissa: s.mantissa(),
            scale: s.scale(),
        }
    }
}
impl From<&Decimal> for BorshDecimal {
    fn from(s: &Decimal) -> Self {
        Self {
            mantissa: s.mantissa(),
            scale: s.scale(),
        }
    }
}
impl From<SwitchboardDecimal> for BorshDecimal {
    fn from(s: SwitchboardDecimal) -> Self {
        Self {
            mantissa: s.mantissa,
            scale: s.scale,
        }
    }
}
impl From<BorshDecimal> for SwitchboardDecimal {
    fn from(val: BorshDecimal) -> Self {
        SwitchboardDecimal {
            mantissa: val.mantissa,
            scale: val.scale,
        }
    }
}
impl TryInto<Decimal> for &BorshDecimal {
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<Decimal> {
        Decimal::try_from_i128_with_scale(self.mantissa, self.scale)
            .map_err(|_| error!(SwitchboardError::DecimalConversionError))
    }
}

impl TryInto<Decimal> for BorshDecimal {
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<Decimal> {
        Decimal::try_from_i128_with_scale(self.mantissa, self.scale)
            .map_err(|_| error!(SwitchboardError::DecimalConversionError))
    }
}

impl SwitchboardDecimal {
    pub fn new(mantissa: i128, scale: u32) -> SwitchboardDecimal {
        Self { mantissa, scale }
    }
    pub fn from_rust_decimal(d: Decimal) -> SwitchboardDecimal {
        Self::new(d.mantissa(), d.scale())
    }
    pub fn from_f64(v: f64) -> SwitchboardDecimal {
        let dec = Decimal::from_f64(v).unwrap();
        Self::from_rust_decimal(dec)
    }
    pub fn scale_to(&self, new_scale: u32) -> i128 {
        match { self.scale }.cmp(&new_scale) {
            std::cmp::Ordering::Greater => self
                .mantissa
                .checked_div(10_i128.pow(self.scale - new_scale))
                .unwrap(),
            std::cmp::Ordering::Less => self
                .mantissa
                .checked_mul(10_i128.pow(new_scale - self.scale))
                .unwrap(),
            std::cmp::Ordering::Equal => self.mantissa,
        }
    }
    pub fn new_with_scale(&self, new_scale: u32) -> Self {
        let mantissa = self.scale_to(new_scale);
        SwitchboardDecimal {
            mantissa,
            scale: new_scale,
        }
    }
}
impl From<Decimal> for SwitchboardDecimal {
    fn from(val: Decimal) -> Self {
        SwitchboardDecimal::new(val.mantissa(), val.scale())
    }
}
impl TryInto<Decimal> for &SwitchboardDecimal {
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<Decimal> {
        Decimal::try_from_i128_with_scale(self.mantissa, self.scale)
            .map_err(|_| error!(SwitchboardError::DecimalConversionError))
    }
}

impl TryInto<Decimal> for SwitchboardDecimal {
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<Decimal> {
        Decimal::try_from_i128_with_scale(self.mantissa, self.scale)
            .map_err(|_| error!(SwitchboardError::DecimalConversionError))
    }
}

impl Ord for SwitchboardDecimal {
    fn cmp(&self, other: &Self) -> Ordering {
        let s: Decimal = self.try_into().unwrap();
        let other: Decimal = other.try_into().unwrap();
        s.cmp(&other)
    }
}

impl PartialOrd for SwitchboardDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
    fn lt(&self, other: &Self) -> bool {
        let s: Decimal = self.try_into().unwrap();
        let other: Decimal = other.try_into().unwrap();
        s < other
    }
    fn le(&self, other: &Self) -> bool {
        let s: Decimal = self.try_into().unwrap();
        let other: Decimal = other.try_into().unwrap();
        s <= other
    }
    fn gt(&self, other: &Self) -> bool {
        let s: Decimal = self.try_into().unwrap();
        let other: Decimal = other.try_into().unwrap();
        s > other
    }
    fn ge(&self, other: &Self) -> bool {
        let s: Decimal = self.try_into().unwrap();
        let other: Decimal = other.try_into().unwrap();
        s >= other
    }
}

impl From<SwitchboardDecimal> for bool {
    fn from(s: SwitchboardDecimal) -> Self {
        let dec: Decimal = (&s).try_into().unwrap();
        dec.round().mantissa() != 0
    }
}

impl TryInto<u64> for SwitchboardDecimal {
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<u64> {
        let dec: Decimal = (&self).try_into().unwrap();
        dec.to_u64()
            .ok_or(error!(SwitchboardError::IntegerOverflowError))
    }
}

impl TryInto<i64> for SwitchboardDecimal {
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<i64> {
        let dec: Decimal = (&self).try_into().unwrap();
        dec.to_i64()
            .ok_or(error!(SwitchboardError::IntegerOverflowError))
    }
}

impl TryInto<f64> for SwitchboardDecimal {
    type Error = anchor_lang::error::Error;
    fn try_into(self) -> anchor_lang::Result<f64> {
        let dec: Decimal = (&self).try_into().unwrap();
        dec.to_f64()
            .ok_or(error!(SwitchboardError::IntegerOverflowError))
    }
}

#[error_code]
#[derive(Eq, PartialEq)]
pub enum SwitchboardError {
    #[msg("Aggregator is not currently populated with a valid round")]
    InvalidAggregatorRound,
    #[msg("Failed to convert string to decimal format")]
    InvalidStrDecimalConversion,
    #[msg("Decimal conversion method failed")]
    DecimalConversionError,
    #[msg("An integer overflow occurred")]
    IntegerOverflowError,
    #[msg("Account discriminator did not match")]
    AccountDiscriminatorMismatch,
    #[msg("Vrf value is empty")]
    VrfEmptyError,
    #[msg("Failed to send requestRandomness instruction")]
    VrfCpiError,
    #[msg("Failed to send signed requestRandomness instruction")]
    VrfCpiSignedError,
    #[msg("Failed to deserialize account")]
    AccountDeserializationError,
    #[msg("Switchboard feed exceeded the staleness threshold")]
    StaleFeed,
    #[msg("Switchboard feed exceeded the confidence interval threshold")]
    ConfidenceIntervalExceeded,
    #[msg("Invalid authority provided to Switchboard account")]
    InvalidAuthority,
    #[msg("Switchboard value variance exceeded threshold")]
    AllowedVarianceExceeded,
    #[msg("Invalid function input")]
    InvalidFunctionInput,
    #[msg("Failed to compute the PDA")]
    PdaDerivationError,
    #[msg("Illegal Operation")]
    IllegalExecuteAttempt,
    #[msg("The provided enclave quote is invalid")]
    InvalidQuote,
    #[msg("The provided queue address did not match the expected address on-chain")]
    InvalidQueue,
    #[msg("The provided enclave_signer does not match the expected enclave_signer")]
    InvalidEnclaveSigner,
    #[msg("The provided mint did not match the wrapped SOL mint address")]
    InvalidNativeMint,
    #[msg("This account has zero mr_enclaves defined")]
    MrEnclavesEmpty,
    InvalidMrEnclave,
    #[msg("The FunctionAccount status is not active (1)")]
    FunctionNotReady,
    #[msg("The FunctionAccount has set requests_disabled to true and disabled this action")]
    UserRequestsDisabled,
    FunctionRoutinesDisabled,
    #[msg(
        "The PermissionAccount is missing the required flags for this action. Check the queues config to see which permissions are required"
    )]
    PermissionDenied,
    ConfigParameterLocked,
    #[msg("The function authority has disabled service execution for this function")]
    FunctionServicesDisabled,
    #[msg("The service has been disabled. Please check the service's is_disabled status for more information.")]
    ServiceDisabled,
    #[msg("The service worker already has the maximum number of services (128)")]
    ServiceWorkerFull,
    #[msg("The service worker is already using its max enclave space for a set of services")]
    ServiceWorkerEnclaveFull,
    #[msg("Service is already being executed by a worker. Please remove the service before adding to a new service worker")]
    ServiceAlreadyAssignedToWorker,
}
