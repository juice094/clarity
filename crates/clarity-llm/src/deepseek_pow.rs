//! DeepSeekHashV1 PoW 求解器（纯 Rust 实现）
//!
//! 算法：SHA3-256 但 `Keccak-f[1600]` 仅执行 rounds 1..23（跳过 round 0）。
//! 服务端预先选定 answer ∈ [0, difficulty)，计算 challenge = hash(prefix + str(answer))，
//! 客户端遍历 [0, difficulty) 找到匹配的 nonce。

use rayon::iter::IntoParallelIterator;
use rayon::prelude::*;

/// Keccak-f[1600] round constants（标准 SHA3 使用 0..23，DeepSeekHashV1 使用 1..23）。
const RC: [u64; 24] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808A,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808B,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008A,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000A,
    0x0000_0000_8000_808B,
    0x8000_0000_0000_008B,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800A,
    0x8000_0000_8000_000A,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];

const RATE: usize = 136;

#[inline(always)]
const fn rotl64(v: u64, k: u32) -> u64 {
    v.wrapping_shl(k) | v.wrapping_shr(64 - k)
}

/// Keccak-f[1600] 置换，执行 rounds 1..23（跳过 round 0）。
fn keccak_f23(state: &mut [u64; 25]) {
    let mut a = *state;

    for r in 1..24 {
        let c0 = a[0] ^ a[5] ^ a[10] ^ a[15] ^ a[20];
        let c1 = a[1] ^ a[6] ^ a[11] ^ a[16] ^ a[21];
        let c2 = a[2] ^ a[7] ^ a[12] ^ a[17] ^ a[22];
        let c3 = a[3] ^ a[8] ^ a[13] ^ a[18] ^ a[23];
        let c4 = a[4] ^ a[9] ^ a[14] ^ a[19] ^ a[24];

        let d0 = c4 ^ rotl64(c1, 1);
        let d1 = c0 ^ rotl64(c2, 1);
        let d2 = c1 ^ rotl64(c3, 1);
        let d3 = c2 ^ rotl64(c4, 1);
        let d4 = c3 ^ rotl64(c0, 1);

        a[0] ^= d0;
        a[5] ^= d0;
        a[10] ^= d0;
        a[15] ^= d0;
        a[20] ^= d0;
        a[1] ^= d1;
        a[6] ^= d1;
        a[11] ^= d1;
        a[16] ^= d1;
        a[21] ^= d1;
        a[2] ^= d2;
        a[7] ^= d2;
        a[12] ^= d2;
        a[17] ^= d2;
        a[22] ^= d2;
        a[3] ^= d3;
        a[8] ^= d3;
        a[13] ^= d3;
        a[18] ^= d3;
        a[23] ^= d3;
        a[4] ^= d4;
        a[9] ^= d4;
        a[14] ^= d4;
        a[19] ^= d4;
        a[24] ^= d4;

        let b0 = a[0];
        let b10 = rotl64(a[1], 1);
        let b20 = rotl64(a[2], 62);
        let b5 = rotl64(a[3], 28);
        let b15 = rotl64(a[4], 27);
        let b16 = rotl64(a[5], 36);
        let b1 = rotl64(a[6], 44);
        let b11 = rotl64(a[7], 6);
        let b21 = rotl64(a[8], 55);
        let b6 = rotl64(a[9], 20);
        let b7 = rotl64(a[10], 3);
        let b17 = rotl64(a[11], 10);
        let b2 = rotl64(a[12], 43);
        let b12 = rotl64(a[13], 25);
        let b22 = rotl64(a[14], 39);
        let b23 = rotl64(a[15], 41);
        let b8 = rotl64(a[16], 45);
        let b18 = rotl64(a[17], 15);
        let b3 = rotl64(a[18], 21);
        let b13 = rotl64(a[19], 8);
        let b14 = rotl64(a[20], 18);
        let b24 = rotl64(a[21], 2);
        let b9 = rotl64(a[22], 61);
        let b19 = rotl64(a[23], 56);
        let b4 = rotl64(a[24], 14);

        a[0] = b0 ^ (!b1 & b2);
        a[1] = b1 ^ (!b2 & b3);
        a[2] = b2 ^ (!b3 & b4);
        a[3] = b3 ^ (!b4 & b0);
        a[4] = b4 ^ (!b0 & b1);
        a[5] = b5 ^ (!b6 & b7);
        a[6] = b6 ^ (!b7 & b8);
        a[7] = b7 ^ (!b8 & b9);
        a[8] = b8 ^ (!b9 & b5);
        a[9] = b9 ^ (!b5 & b6);
        a[10] = b10 ^ (!b11 & b12);
        a[11] = b11 ^ (!b12 & b13);
        a[12] = b12 ^ (!b13 & b14);
        a[13] = b13 ^ (!b14 & b10);
        a[14] = b14 ^ (!b10 & b11);
        a[15] = b15 ^ (!b16 & b17);
        a[16] = b16 ^ (!b17 & b18);
        a[17] = b17 ^ (!b18 & b19);
        a[18] = b18 ^ (!b19 & b15);
        a[19] = b19 ^ (!b15 & b16);
        a[20] = b20 ^ (!b21 & b22);
        a[21] = b21 ^ (!b22 & b23);
        a[22] = b22 ^ (!b23 & b24);
        a[23] = b23 ^ (!b24 & b20);
        a[24] = b24 ^ (!b20 & b21);

        a[0] ^= RC[r as usize];
    }

    *state = a;
}

/// DeepSeekHashV1：SHA3-256 但 Keccak-f 仅执行 rounds 1..23。
pub fn deepseek_hash_v1(data: &[u8]) -> [u8; 32] {
    let mut state = [0u64; 25];
    let mut off = 0;

    while off + RATE <= data.len() {
        for i in 0..RATE / 8 {
            state[i] ^= u64::from_le_bytes([
                data[off + i * 8],
                data[off + i * 8 + 1],
                data[off + i * 8 + 2],
                data[off + i * 8 + 3],
                data[off + i * 8 + 4],
                data[off + i * 8 + 5],
                data[off + i * 8 + 6],
                data[off + i * 8 + 7],
            ]);
        }
        keccak_f23(&mut state);
        off += RATE;
    }

    let mut final_block = [0u8; RATE];
    let tail_len = data.len() - off;
    final_block[..tail_len].copy_from_slice(&data[off..]);
    final_block[tail_len] = 0x06;
    final_block[RATE - 1] |= 0x80;

    for i in 0..RATE / 8 {
        state[i] ^= u64::from_le_bytes([
            final_block[i * 8],
            final_block[i * 8 + 1],
            final_block[i * 8 + 2],
            final_block[i * 8 + 3],
            final_block[i * 8 + 4],
            final_block[i * 8 + 5],
            final_block[i * 8 + 6],
            final_block[i * 8 + 7],
        ]);
    }
    keccak_f23(&mut state);

    let mut out = [0u8; 32];
    out[0..8].copy_from_slice(&state[0].to_le_bytes());
    out[8..16].copy_from_slice(&state[1].to_le_bytes());
    out[16..24].copy_from_slice(&state[2].to_le_bytes());
    out[24..32].copy_from_slice(&state[3].to_le_bytes());
    out
}

/// 构造 PoW 前缀：`{salt}_{expire_at}_`。
pub fn build_pow_prefix(salt: &str, expire_at: u64) -> String {
    format!("{}_{}_", salt, expire_at)
}

/// 将 u64 转为十进制字符串（避免堆分配）。
#[inline]
fn format_nonce(n: u64, buf: &mut [u8; 20]) -> &[u8] {
    if n == 0 {
        buf[19] = b'0';
        return &buf[19..20];
    }
    let mut v = n;
    let mut pos = 20;
    while v > 0 {
        pos -= 1;
        buf[pos] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    &buf[pos..20]
}

/// 解析 64 字符十六进制 challenge 为 4 个 u64（小端）。
fn parse_challenge(challenge_hex: &str) -> Option<[u64; 4]> {
    if challenge_hex.len() != 64 {
        return None;
    }
    let mut target = [0u8; 32];
    for (i, chunk) in target
        .iter_mut()
        .zip(challenge_hex.as_bytes().chunks_exact(2))
    {
        let hi = hex_char_value(chunk[0])?;
        let lo = hex_char_value(chunk[1])?;
        *i = (hi << 4) | lo;
    }
    Some([
        u64_from_le_bytes_8(&target, 0),
        u64_from_le_bytes_8(&target, 8),
        u64_from_le_bytes_8(&target, 16),
        u64_from_le_bytes_8(&target, 24),
    ])
}

/// 预吸收前缀，返回 (base_state, tail, tail_len)。
fn precompute_prefix(salt: &str, expire_at: u64) -> ([u64; 25], [u8; RATE], usize) {
    let prefix = build_pow_prefix(salt, expire_at);
    let prefix_bytes = prefix.as_bytes();

    let mut base_state = [0u64; 25];
    let mut off = 0;
    while off + RATE <= prefix_bytes.len() {
        for i in 0..RATE / 8 {
            base_state[i] ^= u64::from_le_bytes([
                prefix_bytes[off + i * 8],
                prefix_bytes[off + i * 8 + 1],
                prefix_bytes[off + i * 8 + 2],
                prefix_bytes[off + i * 8 + 3],
                prefix_bytes[off + i * 8 + 4],
                prefix_bytes[off + i * 8 + 5],
                prefix_bytes[off + i * 8 + 6],
                prefix_bytes[off + i * 8 + 7],
            ]);
        }
        keccak_f23(&mut base_state);
        off += RATE;
    }

    let tail_len = prefix_bytes.len() - off;
    let mut tail = [0u8; RATE];
    tail[..tail_len].copy_from_slice(&prefix_bytes[off..]);
    (base_state, tail, tail_len)
}

/// 在单个区间 [start, end) 内串行搜索 nonce。
fn search_chunk(
    target: &[u64; 4],
    base_state: &[u64; 25],
    tail: &[u8; RATE],
    tail_len: usize,
    start: u64,
    end: u64,
) -> Option<u64> {
    let mut num_buf = [0u8; 20];
    for n in start..end {
        let nonce_bytes = format_nonce(n, &mut num_buf);
        let num_len = nonce_bytes.len();
        let total_tail = tail_len + num_len;

        let mut s = *base_state;
        if total_tail < RATE {
            let mut buf = [0u8; RATE];
            buf[..tail_len].copy_from_slice(&tail[..tail_len]);
            buf[tail_len..total_tail].copy_from_slice(nonce_bytes);
            buf[total_tail] = 0x06;
            buf[RATE - 1] |= 0x80;
            for i in 0..RATE / 8 {
                s[i] ^= u64::from_le_bytes([
                    buf[i * 8],
                    buf[i * 8 + 1],
                    buf[i * 8 + 2],
                    buf[i * 8 + 3],
                    buf[i * 8 + 4],
                    buf[i * 8 + 5],
                    buf[i * 8 + 6],
                    buf[i * 8 + 7],
                ]);
            }
            keccak_f23(&mut s);
        } else {
            let mut buf = [0u8; RATE];
            buf[..tail_len].copy_from_slice(&tail[..tail_len]);
            let first = RATE - tail_len;
            buf[tail_len..RATE].copy_from_slice(&nonce_bytes[..first]);
            for i in 0..RATE / 8 {
                s[i] ^= u64::from_le_bytes([
                    buf[i * 8],
                    buf[i * 8 + 1],
                    buf[i * 8 + 2],
                    buf[i * 8 + 3],
                    buf[i * 8 + 4],
                    buf[i * 8 + 5],
                    buf[i * 8 + 6],
                    buf[i * 8 + 7],
                ]);
            }
            keccak_f23(&mut s);

            let mut buf2 = [0u8; RATE];
            let rem = total_tail - RATE;
            buf2[..rem].copy_from_slice(&nonce_bytes[first..first + rem]);
            buf2[rem] = 0x06;
            buf2[RATE - 1] |= 0x80;
            for i in 0..RATE / 8 {
                s[i] ^= u64::from_le_bytes([
                    buf2[i * 8],
                    buf2[i * 8 + 1],
                    buf2[i * 8 + 2],
                    buf2[i * 8 + 3],
                    buf2[i * 8 + 4],
                    buf2[i * 8 + 5],
                    buf2[i * 8 + 6],
                    buf2[i * 8 + 7],
                ]);
            }
            keccak_f23(&mut s);
        }

        if s[0] == target[0] && s[1] == target[1] && s[2] == target[2] && s[3] == target[3] {
            return Some(n);
        }
    }
    None
}

/// 串行求解 PoW。
///
/// 在 [0, difficulty) 范围内搜索 nonce，使得
/// `DeepSeekHashV1(prefix + decimal(nonce)) == challenge_hex`。
pub fn solve_pow(challenge_hex: &str, salt: &str, expire_at: u64, difficulty: u64) -> Option<u64> {
    let target = parse_challenge(challenge_hex)?;
    let (base_state, tail, tail_len) = precompute_prefix(salt, expire_at);
    search_chunk(&target, &base_state, &tail, tail_len, 0, difficulty)
}

/// 并行求解 PoW。
///
/// 当 `difficulty` 较大时按 CPU 核心数分块搜索，找到第一个匹配即返回。
pub fn solve_pow_parallel(
    challenge_hex: &str,
    salt: &str,
    expire_at: u64,
    difficulty: u64,
) -> Option<u64> {
    let target = parse_challenge(challenge_hex)?;
    let (base_state, tail, tail_len) = precompute_prefix(salt, expire_at);

    let threads = rayon::current_num_threads() as u64;
    let chunk_size = difficulty.div_ceil(threads);

    (0..threads).into_par_iter().find_map_any(|t| {
        let start = t * chunk_size;
        if start >= difficulty {
            return None;
        }
        let end = ((t + 1) * chunk_size).min(difficulty);
        search_chunk(&target, &base_state, &tail, tail_len, start, end)
    })
}

/// 自动选择串行或并行求解。
///
/// 小 difficulty 直接串行，避免 rayon 线程池调度开销；
/// 大 difficulty 使用多核并行。
pub fn solve_pow_auto(
    challenge_hex: &str,
    salt: &str,
    expire_at: u64,
    difficulty: u64,
) -> Option<u64> {
    // 阈值基于 release 模式下的实测：difficulty 144000 时并行比串行快数倍；
    // debug 模式下 rayon 线程池启动开销会抵消收益，但生产环境使用 release。
    const PARALLEL_THRESHOLD: u64 = 50_000;
    if difficulty < PARALLEL_THRESHOLD {
        solve_pow(challenge_hex, salt, expire_at, difficulty)
    } else {
        solve_pow_parallel(challenge_hex, salt, expire_at, difficulty)
    }
}

#[inline(always)]
const fn u64_from_le_bytes_8(bytes: &[u8; 32], start: usize) -> u64 {
    u64::from_le_bytes([
        bytes[start],
        bytes[start + 1],
        bytes[start + 2],
        bytes[start + 3],
        bytes[start + 4],
        bytes[start + 5],
        bytes[start + 6],
        bytes[start + 7],
    ])
}

#[inline(always)]
const fn hex_char_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn test_deepseek_hash_v1_vectors() {
        let cases = [
            (
                "",
                "e594808bc5b7151ac160c6d39a02e0a8e261ed588578403099e3561dc40c26b3",
            ),
            (
                "testsalt_1700000000_42",
                "d4a2ea58c89e40887c933484868380c6f803eaa8dc53a3b9df8e431b921a4f09",
            ),
            (
                "testsalt_1700000000_100000",
                "abea2f35796b65486e9be1b36f7878c66cab021e96faa473fdf4decd31f9ba30",
            ),
            (
                "abc123salt_1700000000_12345",
                "74b3b7452745b70e85eb32ee7f0a9ec0381d42dd5137b695da915e104fc390e1",
            ),
        ];
        for (input, expected) in cases {
            let hash = deepseek_hash_v1(input.as_bytes());
            assert_eq!(hex_encode(&hash), expected, "input: {}", input);
        }
    }

    #[test]
    fn test_solve_pow() {
        let cases = [
            ("testsalt", 1700000000u64, 42u64, 1000u64),
            ("testsalt", 1700000000, 500, 2000),
            ("abc123salt", 1700000000, 12345, 20000),
        ];
        for (salt, expire, answer, diff) in cases {
            let input = build_pow_prefix(salt, expire) + &answer.to_string();
            let challenge = hex_encode(&deepseek_hash_v1(input.as_bytes()));
            let got = solve_pow(&challenge, salt, expire, diff);
            assert_eq!(got, Some(answer), "salt={} answer={}", salt, answer);
        }
    }

    #[test]
    fn test_solve_pow_parallel() {
        let cases = [
            ("testsalt", 1700000000u64, 42u64, 1000u64),
            ("testsalt", 1700000000, 500, 2000),
            ("abc123salt", 1700000000, 12345, 20000),
            ("largesalt", 1700000000, 80000, 144000),
        ];
        for (salt, expire, answer, diff) in cases {
            let input = build_pow_prefix(salt, expire) + &answer.to_string();
            let challenge = hex_encode(&deepseek_hash_v1(input.as_bytes()));
            let got = solve_pow_parallel(&challenge, salt, expire, diff);
            assert_eq!(
                got,
                Some(answer),
                "parallel salt={} answer={}",
                salt,
                answer
            );
        }
    }

    #[test]
    fn test_solve_pow_auto() {
        let input = build_pow_prefix("testsalt", 1700000000) + "500";
        let challenge = hex_encode(&deepseek_hash_v1(input.as_bytes()));
        assert_eq!(
            solve_pow_auto(&challenge, "testsalt", 1700000000, 2000),
            Some(500)
        );
    }

    #[test]
    fn test_solve_pow_not_found() {
        // difficulty 太小，找不到答案
        let input = build_pow_prefix("testsalt", 1700000000) + "500";
        let challenge = hex_encode(&deepseek_hash_v1(input.as_bytes()));
        let got = solve_pow(&challenge, "testsalt", 1700000000, 100);
        assert_eq!(got, None);
    }

    #[test]
    fn test_build_prefix() {
        assert_eq!(build_pow_prefix("salt", 1712345678), "salt_1712345678_");
    }

    /// 默认 difficulty（144000）性能测试，默认不运行。
    #[test]
    #[ignore]
    fn bench_solve_default_difficulty() {
        let answer = 72_000u64;
        let input = build_pow_prefix("realisticsalt", 1_712_345_678) + &answer.to_string();
        let challenge = hex_encode(&deepseek_hash_v1(input.as_bytes()));
        let start = std::time::Instant::now();
        let got = solve_pow(&challenge, "realisticsalt", 1_712_345_678, 144_000);
        let elapsed = start.elapsed();
        println!("solved in {:?}", elapsed);
        assert_eq!(got, Some(answer));
    }
}
