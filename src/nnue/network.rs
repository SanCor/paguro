//! Stockfish 14 NNUE file loader and hidden-layer forward pass.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub const FT_IN: usize = 45056;
pub const FT_OUT: usize = 512;
pub const PSQT_BUCKETS: usize = 8;
pub const LAYER_STACKS: usize = 8;
pub const VERSION: u32 = 0x7AF32F20;
pub const FEATURE_HASH: u32 = 0x5F234CB8;
pub const FT_HASH: u32 = FEATURE_HASH ^ ((FT_OUT * 2) as u32);

pub struct LayerStack {
    pub fc0_bias: [i32; 16],
    pub fc0_weights: Vec<i8>, // 16 * 1024
    pub fc1_bias: [i32; 32],
    pub fc1_weights: Vec<i8>, // 32 * 32 padded
    pub fc2_bias: i32,
    pub fc2_weights: [i8; 32],
}

pub struct Network {
    pub ft_biases: Vec<i16>,
    pub ft_weights: Vec<i16>,
    pub psqt: Vec<i32>,
    pub stacks: Vec<LayerStack>,
}

impl Network {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Network> {
        let mut file = File::open(path.as_ref())?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Network::from_bytes(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn from_bytes(data: &[u8]) -> Result<Network, String> {
        let mut r = Reader { data, pos: 0 };
        let version = r.u32()?;
        if version != VERSION {
            return Err(format!("bad nnue version {version:#X}"));
        }
        let _hash = r.u32()?;
        let desc_len = r.u32()? as usize;
        r.skip(desc_len)?;

        let ft_hash = r.u32()?;
        if ft_hash != FT_HASH {
            return Err(format!("bad ft hash {ft_hash:#X}, expected {FT_HASH:#X}"));
        }

        let ft_biases = r.i16s(FT_OUT)?;
        let ft_weights = r.i16s(FT_OUT * FT_IN)?;
        let psqt = r.i32s(FT_IN * PSQT_BUCKETS)?;

        let mut stacks = Vec::with_capacity(LAYER_STACKS);
        for _ in 0..LAYER_STACKS {
            let _nh = r.u32()?;
            let fc0_bias = r.i32_arr16()?;
            let fc0_weights = r.i8s(16 * 1024)?;
            let fc1_bias = r.i32_arr32()?;
            let fc1_weights = r.i8s(32 * 32)?;
            let fc2_bias = r.i32()?;
            let mut fc2_weights = [0i8; 32];
            let w = r.i8s(32)?;
            fc2_weights.copy_from_slice(&w);
            stacks.push(LayerStack {
                fc0_bias,
                fc0_weights,
                fc1_bias,
                fc1_weights,
                fc2_bias,
                fc2_weights,
            });
        }

        Ok(Network {
            ft_biases,
            ft_weights,
            psqt,
            stacks,
        })
    }

    pub fn propagate(&self, bucket: usize, input: &[u8; 1024]) -> i32 {
        let s = &self.stacks[bucket];
        let mut h1 = [0u8; 16];
        crate::nnue::simd::fc0_relu(&s.fc0_weights, input, &s.fc0_bias, &mut h1);

        let mut h2 = [0u8; 32];
        crate::nnue::simd::fc1_relu(&s.fc1_weights, &h1, &s.fc1_bias, &mut h2);
        crate::nnue::simd::fc2(&s.fc2_weights, &h2, s.fc2_bias)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn need(&self, n: usize) -> Result<(), String> {
        if self.pos + n <= self.data.len() {
            Ok(())
        } else {
            Err("unexpected eof in nnue".into())
        }
    }

    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.need(n)?;
        self.pos += n;
        Ok(())
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.need(4)?;
        let v = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    fn i32(&mut self) -> Result<i32, String> {
        self.need(4)?;
        let v = i32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    fn i16s(&mut self, n: usize) -> Result<Vec<i16>, String> {
        self.need(n * 2)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(i16::from_le_bytes(
                self.data[self.pos..self.pos + 2].try_into().unwrap(),
            ));
            self.pos += 2;
        }
        Ok(v)
    }

    fn i32s(&mut self, n: usize) -> Result<Vec<i32>, String> {
        self.need(n * 4)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(i32::from_le_bytes(
                self.data[self.pos..self.pos + 4].try_into().unwrap(),
            ));
            self.pos += 4;
        }
        Ok(v)
    }

    fn i8s(&mut self, n: usize) -> Result<Vec<i8>, String> {
        self.need(n)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(self.data[self.pos] as i8);
            self.pos += 1;
        }
        Ok(v)
    }

    fn i32_arr16(&mut self) -> Result<[i32; 16], String> {
        let mut a = [0i32; 16];
        for i in 0..16 {
            a[i] = self.i32()?;
        }
        Ok(a)
    }

    fn i32_arr32(&mut self) -> Result<[i32; 32], String> {
        let mut a = [0i32; 32];
        for i in 0..32 {
            a[i] = self.i32()?;
        }
        Ok(a)
    }
}
