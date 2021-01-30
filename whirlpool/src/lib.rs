//! An implementation of the [Whirlpool][1] cryptographic hash algorithm.
//!
//! This is the algorithm recommended by NESSIE (New European Schemes for
//! Signatures, Integrity and Encryption; an European research project).
//!
//! The constants used by Whirlpool were changed twice (2001 and 2003) - this
//! crate only implements the most recent standard. The two older Whirlpool
//! implementations (sometimes called Whirlpool-0 (pre 2001) and Whirlpool-T
//! (pre 2003)) were not used much anyway (both have never been recommended
//! by NESSIE).
//!
//! For details see <http://www.larc.usp.br/~pbarreto/WhirlpoolPage.html>.
//!
//! # Usage
//!
//! ```rust
//! use whirlpool::{Whirlpool, Digest};
//! use hex_literal::hex;
//!
//! // create a hasher object, to use it do not forget to import `Digest` trait
//! let mut hasher = Whirlpool::new();
//! // write input message
//! hasher.update(b"Hello Whirlpool");
//! // read hash digest (it will consume hasher)
//! let result = hasher.finalize();
//!
//! assert_eq!(result[..], hex!("
//!     8eaccdc136903c458ea0b1376be2a5fc9dc5b8ce8892a3b4f43366e2610c206c
//!     a373816495e63db0fff2ff25f75aa7162f332c9f518c3036456502a8414d300a
//! ")[..]);
//! ```
//!
//! Also see [RustCrypto/hashes][2] readme.
//!
//! [1]: https://en.wikipedia.org/wiki/Whirlpool_(hash_function)
//! [2]: https://github.com/RustCrypto/hashes

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![warn(missing_docs, rust_2018_idioms)]

pub use digest::{self, Digest};

#[cfg(not(feature = "asm"))]
mod compress;

#[cfg(feature = "asm")]
use whirlpool_asm as compress;

use compress::compress;

use core::{fmt, slice::from_ref};
use digest::{
    block_buffer::BlockBuffer,
    consts::U64,
    core_api::{AlgorithmName, FixedOutputCore, UpdateCore, UpdateCoreWrapper},
    generic_array::{typenum::Unsigned, GenericArray},
};

type BlockSize = U64;
type Block = GenericArray<u8, BlockSize>;
const BLOCK_SIZE: usize = BlockSize::USIZE;

/// Core Whirlpool hasher state.
#[derive(Clone)]
pub struct WhirlpoolCore {
    bit_len: [u64; 4],
    state: [u64; 8],
}

impl UpdateCore for WhirlpoolCore {
    type BlockSize = BlockSize;

    #[inline]
    fn update_blocks(&mut self, blocks: &[Block]) {
        let block_bits = 8 * BLOCK_SIZE as u64;
        self.update_len(block_bits * (blocks.len() as u64));
        compress(&mut self.state, convert(blocks));
    }
}

impl FixedOutputCore for WhirlpoolCore {
    type OutputSize = U64;

    #[inline]
    fn finalize_fixed_core(
        &mut self,
        buffer: &mut BlockBuffer<Self::BlockSize>,
        out: &mut GenericArray<u8, Self::OutputSize>,
    ) {
        let pos = buffer.get_pos();
        self.update_len(8 * pos as u64);

        let mut buf = [0u8; 4 * 8];
        for (chunk, v) in buf.chunks_exact_mut(8).zip(self.bit_len.iter()) {
            chunk.copy_from_slice(&v.to_be_bytes());
        }

        let mut state = self.state;
        buffer.digest_pad(&buf, |block| compress(&mut state, convert(from_ref(block))));

        for (chunk, v) in out.chunks_exact_mut(8).zip(state.iter()) {
            chunk.copy_from_slice(&v.to_le_bytes());
        }
    }
}

impl WhirlpoolCore {
    fn update_len(&mut self, len: u64) {
        let mut carry = 0;
        adc(&mut self.bit_len[3], len, &mut carry);
        adc(&mut self.bit_len[2], 0, &mut carry);
        adc(&mut self.bit_len[1], 0, &mut carry);
        adc(&mut self.bit_len[0], 0, &mut carry);
    }
}

impl Default for WhirlpoolCore {
    #[inline]
    fn default() -> Self {
        Self {
            bit_len: Default::default(),
            state: [0u64; 8],
        }
    }
}

impl AlgorithmName for WhirlpoolCore {
    fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WhirlpoolCore")
    }
}

impl fmt::Debug for WhirlpoolCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WhirlpoolCore { ... }")
    }
}

/// Whirlpool hasher state.
pub type Whirlpool = UpdateCoreWrapper<WhirlpoolCore>;

#[inline(always)]
fn adc(a: &mut u64, b: u64, carry: &mut u64) {
    let ret = (*a as u128) + (b as u128) + (*carry as u128);
    *a = ret as u64;
    *carry = (ret >> 64) as u64;
}

#[inline(always)]
fn convert(blocks: &[Block]) -> &[[u8; BLOCK_SIZE]] {
    // SAFETY: GenericArray<u8, U64> and [u8; 64] have
    // exactly the same memory layout
    let p = blocks.as_ptr() as *const [u8; BLOCK_SIZE];
    unsafe { core::slice::from_raw_parts(p, blocks.len()) }
}
