//! Xray/V2Ray geo databases (`geosite.dat`, `geoip.dat`) -> sing-box rule-sets.
//!
//! sing-box 1.12 no longer reads `.dat` files, so routing entries such as
//! `geosite:category-ru` / `geoip:ru` are converted into local *source* rule-sets.
//! Xray reads the `.dat` files natively but refuses to start when a referenced code is
//! missing, so [`list_codes`] lets callers validate codes up front.
//!
//! The files are protobuf (`app/router/config.proto` in v2fly/xray). The decoder below is
//! hand-rolled: the whole file is read into memory once and entries that were not
//! requested are skipped without allocating.

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct GeoSiteSet {
    pub full: Vec<String>,    // Type::Full
    pub suffix: Vec<String>,  // Type::Domain
    pub keyword: Vec<String>, // Type::Plain
    pub regex: Vec<String>,   // Type::Regex
}

/// Placeholder values that never match, so an empty rule-set is still valid for sing-box.
const EMPTY_DOMAIN: &str = "nexvpn-empty.invalid";
const EMPTY_CIDR: &str = "255.255.255.255/32";

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const DOWNLOAD_MIN_BYTES: u64 = 1024;
const DOWNLOAD_MAX_BYTES: u64 = 256 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Minimal protobuf reader
// ---------------------------------------------------------------------------

const WT_VARINT: u8 = 0;
const WT_I64: u8 = 1;
const WT_LEN: u8 = 2;
const WT_I32: u8 = 5;

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn varint(&mut self) -> Result<u64> {
        let mut value = 0u64;
        for i in 0..10 {
            let b = *self
                .buf
                .get(self.pos)
                .ok_or_else(|| anyhow!("truncated varint"))?;
            self.pos += 1;
            // The 10th byte may only carry the top bit of a u64.
            if i == 9 && b > 1 {
                bail!("varint overflows 64 bits");
            }
            value |= u64::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(value);
            }
        }
        unreachable!("10th varint byte either terminates or bails")
    }

    fn advance(&mut self, n: usize) -> Result<()> {
        if self.buf.len() - self.pos < n {
            bail!("truncated fixed-size field");
        }
        self.pos += n;
        Ok(())
    }

    /// Length-delimited payload (wire type 2), borrowed from the buffer.
    fn bytes(&mut self) -> Result<&'a [u8]> {
        let len = usize::try_from(self.varint()?).map_err(|_| anyhow!("field too long"))?;
        if self.buf.len() - self.pos < len {
            bail!("truncated length-delimited field");
        }
        let out = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(out)
    }

    fn skip(&mut self, wire: u8) -> Result<()> {
        match wire {
            WT_VARINT => self.varint().map(drop),
            WT_I64 => self.advance(8),
            WT_LEN => self.bytes().map(drop),
            WT_I32 => self.advance(4),
            other => bail!("unsupported protobuf wire type {other}"),
        }
    }

    /// Next field header as (field number, wire type); `None` at end of buffer.
    fn field(&mut self) -> Result<Option<(u64, u8)>> {
        if self.pos >= self.buf.len() {
            return Ok(None);
        }
        let key = self.varint()?;
        let field = key >> 3;
        if field == 0 {
            bail!("invalid protobuf field number 0");
        }
        Ok(Some((field, (key & 7) as u8)))
    }
}

/// Walk a `GeoSiteList` / `GeoIPList`, calling `f(country_code, entry_bytes)` for every
/// entry. `f` returns `false` to stop early.
fn for_each_entry<'a>(
    data: &'a [u8],
    mut f: impl FnMut(&'a [u8], &'a [u8]) -> Result<bool>,
) -> Result<()> {
    let mut r = Reader::new(data);
    while let Some((field, wire)) = r.field()? {
        if field == 1 && wire == WT_LEN {
            let entry = r.bytes()?;
            if !f(entry_code(entry)?, entry)? {
                break;
            }
        } else {
            r.skip(wire)?;
        }
    }
    Ok(())
}

/// `country_code` (field 1) of a GeoSite / GeoIP message. Other fields are only skipped
/// over (O(1) each), nothing is decoded.
fn entry_code(entry: &[u8]) -> Result<&[u8]> {
    let mut r = Reader::new(entry);
    let mut code: &[u8] = &[];
    while let Some((field, wire)) = r.field()? {
        if field == 1 && wire == WT_LEN {
            code = r.bytes()?;
        } else {
            r.skip(wire)?;
        }
    }
    Ok(code)
}

/// Lowercase raw code bytes into a reusable buffer (no allocation once warmed up).
fn lower_into(buf: &mut String, bytes: &[u8]) {
    buf.clear();
    for c in String::from_utf8_lossy(bytes).chars() {
        buf.extend(c.to_lowercase());
    }
}

/// Case-insensitive compare of raw bytes against an already-lowercased string.
fn eq_lower(raw: &[u8], lowered: &str) -> bool {
    String::from_utf8_lossy(raw)
        .chars()
        .flat_map(char::to_lowercase)
        .eq(lowered.chars())
}

fn read_dat(dat: &Path) -> Result<Vec<u8>> {
    std::fs::read(dat).with_context(|| format!("failed to read {}", dat.display()))
}

// ---------------------------------------------------------------------------
// Public readers
// ---------------------------------------------------------------------------

/// Lowercased codes present in a geosite.dat or geoip.dat file.
#[allow(dead_code)]
pub fn list_codes(dat: &Path) -> Result<HashSet<String>> {
    let data = read_dat(dat)?;
    let mut codes = HashSet::new();
    let mut lc = String::new();
    for_each_entry(&data, |code, _| {
        lower_into(&mut lc, code);
        if !lc.is_empty() && !codes.contains(lc.as_str()) {
            codes.insert(lc.clone());
        }
        Ok(true)
    })
    .with_context(|| format!("failed to parse {}", dat.display()))?;
    Ok(codes)
}

struct AttrFilter {
    attr: String, // lowercased
    negate: bool,
}

/// One requested geosite code: map key plus its `@attr` filters (all must hold).
struct SiteRequest {
    key: String,
    filters: Vec<AttrFilter>,
}

impl SiteRequest {
    /// Split a lowercased "code@attr@!attr2" into (base code, request).
    fn parse(key: String) -> (String, Self) {
        let mut parts = key.split('@');
        let base = parts.next().unwrap_or_default().trim().to_string();
        let filters = parts
            .filter_map(|p| {
                let p = p.trim();
                let (negate, attr) = match p.strip_prefix('!') {
                    Some(rest) => (true, rest.trim()),
                    None => (false, p),
                };
                // Xray ignores empty attributes ("google@" == "google").
                (!attr.is_empty()).then(|| AttrFilter {
                    attr: attr.to_string(),
                    negate,
                })
            })
            .collect();
        (base, Self { key, filters })
    }

    fn matches(&self, attr_keys: &[&[u8]]) -> bool {
        self.filters.iter().all(|f| {
            let has = attr_keys.iter().any(|k| eq_lower(k, &f.attr));
            has != f.negate
        })
    }
}

/// Extract the requested geosite categories. `codes` are case-insensitive and may carry
/// an attribute filter "code@attr" (keep only domains that have attribute key == attr,
/// case-insensitive; a leading '!' on attr, "code@!attr", means domains WITHOUT it).
/// Result is keyed by the requested code string lowercased (including "@attr").
/// Codes not present in the file are simply absent from the map.
#[allow(dead_code)]
pub fn read_geosite(dat: &Path, codes: &[String]) -> Result<HashMap<String, GeoSiteSet>> {
    // base code -> every requested variant of it ("google", "google@cn", ...)
    let mut pending: HashMap<String, Vec<SiteRequest>> = HashMap::new();
    let mut seen = HashSet::new();
    for code in codes {
        let key = code.to_lowercase();
        if seen.insert(key.clone()) {
            let (base, req) = SiteRequest::parse(key);
            pending.entry(base).or_default().push(req);
        }
    }
    let mut out = HashMap::new();
    if pending.is_empty() {
        return Ok(out);
    }

    let data = read_dat(dat)?;
    let mut lc = String::new();
    for_each_entry(&data, |code, entry| {
        lower_into(&mut lc, code);
        // First entry with a given code wins (same as Xray); stop once all are found.
        if let Some(reqs) = pending.remove(lc.as_str()) {
            let sets = parse_geosite_entry(entry, &reqs)
                .with_context(|| format!("geosite entry '{lc}'"))?;
            for (req, set) in reqs.into_iter().zip(sets) {
                out.insert(req.key, set);
            }
        }
        Ok(!pending.is_empty())
    })
    .with_context(|| format!("failed to parse {}", dat.display()))?;
    Ok(out)
}

fn parse_geosite_entry(entry: &[u8], reqs: &[SiteRequest]) -> Result<Vec<GeoSiteSet>> {
    let mut sets = vec![GeoSiteSet::default(); reqs.len()];
    let need_attrs = reqs.iter().any(|r| !r.filters.is_empty());
    let mut attr_keys: Vec<&[u8]> = Vec::new();
    let mut skipped = 0usize;

    let mut r = Reader::new(entry);
    while let Some((field, wire)) = r.field()? {
        if field != 2 || wire != WT_LEN {
            r.skip(wire)?;
            continue;
        }
        let mut d = Reader::new(r.bytes()?);
        let mut kind = 0u64; // proto3 default: Plain
        let mut value: &[u8] = &[];
        attr_keys.clear();
        while let Some((field, wire)) = d.field()? {
            match (field, wire) {
                (1, WT_VARINT) => kind = d.varint()?,
                (2, WT_LEN) => value = d.bytes()?,
                (3, WT_LEN) if need_attrs => attr_keys.push(attribute_key(d.bytes()?)?),
                _ => d.skip(wire)?,
            }
        }
        // An empty value would be a match-everything keyword; unknown types can't be mapped.
        let value = match std::str::from_utf8(value) {
            Ok(v) if !v.is_empty() && kind <= 3 => v,
            _ => {
                skipped += 1;
                continue;
            }
        };
        for (req, set) in reqs.iter().zip(sets.iter_mut()) {
            if !req.matches(&attr_keys) {
                continue;
            }
            let list = match kind {
                0 => &mut set.keyword,
                1 => &mut set.regex,
                2 => &mut set.suffix,
                _ => &mut set.full,
            };
            list.push(value.to_owned());
        }
    }
    if skipped > 0 {
        log::warn!("geosite: skipped {skipped} domain(s) with empty/invalid value or unknown type");
    }
    Ok(sets)
}

/// `key` (field 1) of an Attribute message.
fn attribute_key(buf: &[u8]) -> Result<&[u8]> {
    let mut r = Reader::new(buf);
    let mut key: &[u8] = &[];
    while let Some((field, wire)) = r.field()? {
        if field == 1 && wire == WT_LEN {
            key = r.bytes()?;
        } else {
            r.skip(wire)?;
        }
    }
    Ok(key)
}

/// Extract CIDRs ("1.2.3.0/24", "2001:db8::/32") for the requested geoip codes
/// (case-insensitive, keyed lowercased). Entries with reverse_match=true: log::warn and
/// return them as-is (not inverted). Missing codes are absent from the map.
#[allow(dead_code)]
pub fn read_geoip(dat: &Path, codes: &[String]) -> Result<HashMap<String, Vec<String>>> {
    let mut pending: HashSet<String> = codes.iter().map(|c| c.to_lowercase()).collect();
    let mut out = HashMap::new();
    if pending.is_empty() {
        return Ok(out);
    }

    let data = read_dat(dat)?;
    let mut lc = String::new();
    for_each_entry(&data, |code, entry| {
        lower_into(&mut lc, code);
        if pending.remove(lc.as_str()) {
            let cidrs =
                parse_geoip_entry(entry, &lc).with_context(|| format!("geoip entry '{lc}'"))?;
            out.insert(lc.clone(), cidrs);
        }
        Ok(!pending.is_empty())
    })
    .with_context(|| format!("failed to parse {}", dat.display()))?;
    Ok(out)
}

fn parse_geoip_entry(entry: &[u8], code: &str) -> Result<Vec<String>> {
    let mut cidrs = Vec::new();
    let mut reverse = false;
    let mut skipped = 0usize;
    let mut r = Reader::new(entry);
    while let Some((field, wire)) = r.field()? {
        match (field, wire) {
            (2, WT_LEN) => match parse_cidr(r.bytes()?)? {
                Some(c) => cidrs.push(c),
                None => skipped += 1,
            },
            (3, WT_VARINT) => reverse = r.varint()? != 0,
            _ => r.skip(wire)?,
        }
    }
    if reverse {
        log::warn!(
            "geoip '{code}' has reverse_match=true; its CIDRs are used as-is (not inverted)"
        );
    }
    if skipped > 0 {
        log::warn!("geoip '{code}': skipped {skipped} CIDR(s) with invalid address or prefix");
    }
    Ok(cidrs)
}

/// CIDR message -> "addr/prefix" with host bits cleared; `None` if malformed.
fn parse_cidr(buf: &[u8]) -> Result<Option<String>> {
    let mut r = Reader::new(buf);
    let mut ip: &[u8] = &[];
    let mut prefix = 0u64;
    while let Some((field, wire)) = r.field()? {
        match (field, wire) {
            (1, WT_LEN) => ip = r.bytes()?,
            (2, WT_VARINT) => prefix = r.varint()?,
            _ => r.skip(wire)?,
        }
    }
    Ok(match (<[u8; 4]>::try_from(ip), <[u8; 16]>::try_from(ip)) {
        (Ok(v4), _) if prefix <= 32 => {
            let mask = u32::MAX.checked_shl(32 - prefix as u32).unwrap_or(0);
            let addr = Ipv4Addr::from(u32::from_be_bytes(v4) & mask);
            Some(format!("{addr}/{prefix}"))
        }
        (_, Ok(v6)) if prefix <= 128 => {
            let mask = u128::MAX.checked_shl(128 - prefix as u32).unwrap_or(0);
            let addr = Ipv6Addr::from(u128::from_be_bytes(v6) & mask);
            Some(format!("{addr}/{prefix}"))
        }
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// sing-box rule-sets
// ---------------------------------------------------------------------------

fn rule_set_tag(prefix: &str, code: &str) -> String {
    let mut tag = String::with_capacity(prefix.len() + code.len() + 8);
    tag.push_str(prefix);
    for c in code.to_lowercase().chars() {
        match c {
            '@' => tag.push_str("-at-"),
            '!' => tag.push_str("not-"),
            'a'..='z' | '0'..='9' | '-' | '_' | '.' => tag.push(c),
            _ => tag.push('_'),
        }
    }
    tag
}

/// sing-box rule-set tag for a geosite code: "geosite-<code>", lowercased,
/// '@' -> '-at-', '!' -> 'not-', any other char not [a-z0-9-_.] -> '_'.
#[allow(dead_code)]
pub fn geosite_tag(code: &str) -> String {
    rule_set_tag("geosite-", code)
}

/// sing-box rule-set tag for a geoip code: "geoip-<code>" (same escaping as geosite_tag).
#[allow(dead_code)]
pub fn geoip_tag(code: &str) -> String {
    rule_set_tag("geoip-", code)
}

#[derive(Serialize)]
struct SourceRuleSet<R> {
    version: u8,
    rules: [R; 1],
}

fn is_empty(v: &&[String]) -> bool {
    v.is_empty()
}

#[derive(Serialize)]
struct DomainRule<'a> {
    #[serde(skip_serializing_if = "is_empty")]
    domain: &'a [String],
    #[serde(skip_serializing_if = "is_empty")]
    domain_suffix: &'a [String],
    #[serde(skip_serializing_if = "is_empty")]
    domain_keyword: &'a [String],
    #[serde(skip_serializing_if = "is_empty")]
    domain_regex: &'a [String],
}

#[derive(Serialize)]
struct IpRule<'a> {
    ip_cidr: &'a [String],
}

fn geosite_rule_set_json(set: &GeoSiteSet) -> Result<Vec<u8>> {
    let empty = [EMPTY_DOMAIN.to_string()];
    let is_empty = set.full.is_empty()
        && set.suffix.is_empty()
        && set.keyword.is_empty()
        && set.regex.is_empty();
    let rule = DomainRule {
        domain: if is_empty { &empty } else { &set.full },
        domain_suffix: &set.suffix,
        domain_keyword: &set.keyword,
        domain_regex: &set.regex,
    };
    Ok(serde_json::to_vec(&SourceRuleSet {
        version: 2,
        rules: [rule],
    })?)
}

fn geoip_rule_set_json(cidrs: &[String]) -> Result<Vec<u8>> {
    let empty = [EMPTY_CIDR.to_string()];
    let rule = IpRule {
        ip_cidr: if cidrs.is_empty() { &empty } else { cidrs },
    };
    Ok(serde_json::to_vec(&SourceRuleSet {
        version: 2,
        rules: [rule],
    })?)
}

fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path_with_suffix(path, ".tmp");
    std::fs::write(&tmp, data).with_context(|| format!("failed to write {}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("failed to write {}", path.display()));
    }
    Ok(())
}

/// Cached rule-set is reusable when it is strictly newer than the source .dat.
fn is_fresh(path: &Path, dat_mtime: Option<SystemTime>) -> bool {
    let (Some(dat_mtime), Ok(meta)) = (dat_mtime, std::fs::metadata(path)) else {
        return false;
    };
    meta.is_file() && meta.len() > 0 && meta.modified().is_ok_and(|m| m > dat_mtime)
}

/// Resolve the .dat for one kind; errors if codes are requested but the file is unusable.
fn require_dat<'a>(kind: &str, dat: Option<&'a Path>) -> Result<(&'a Path, Option<SystemTime>)> {
    let dat = dat.ok_or_else(|| anyhow!("{kind} codes requested but no {kind}.dat path given"))?;
    let meta = std::fs::metadata(dat)
        .ok()
        .filter(|m| m.is_file())
        .ok_or_else(|| anyhow!("{kind}.dat not found at {}", dat.display()))?;
    Ok((dat, meta.modified().ok()))
}

/// (lowercased code, tag) pairs in request order, without duplicate tags.
fn unique_codes(codes: &[String], tag_fn: fn(&str) -> String) -> Vec<(String, String)> {
    let mut seen = HashMap::<String, String>::new();
    let mut out = Vec::new();
    for code in codes {
        let key = code.to_lowercase();
        let tag = tag_fn(&key);
        match seen.get(&tag) {
            Some(prev) if *prev != key => {
                log::warn!("rule-set tag '{tag}' of '{key}' collides with '{prev}', skipping");
            }
            Some(_) => {}
            None => {
                seen.insert(tag.clone(), key.clone());
                out.push((key, tag));
            }
        }
    }
    out
}

/// Shared cache/read/write loop for one kind of rule-set.
fn build_kind<T>(
    kind: &str,
    dat: Option<&Path>,
    codes: &[String],
    tag_fn: fn(&str) -> String,
    out_dir: &Path,
    read: impl FnOnce(&Path, &[String]) -> Result<HashMap<String, T>>,
    to_json: impl Fn(&T) -> Result<Vec<u8>>,
) -> Result<Vec<(String, PathBuf)>> {
    let mut result = Vec::new();
    let codes = unique_codes(codes, tag_fn);
    if codes.is_empty() {
        return Ok(result);
    }
    let (dat, dat_mtime) = require_dat(kind, dat)?;
    let entries: Vec<(String, String, PathBuf, bool)> = codes
        .into_iter()
        .map(|(key, tag)| {
            let path = out_dir.join(format!("{tag}.json"));
            let fresh = is_fresh(&path, dat_mtime);
            (key, tag, path, fresh)
        })
        .collect();

    let stale: Vec<String> = entries
        .iter()
        .filter(|e| !e.3)
        .map(|e| e.0.clone())
        .collect();
    let data = if stale.is_empty() {
        HashMap::new()
    } else {
        read(dat, &stale)?
    };

    for (key, tag, path, fresh) in entries {
        if !fresh {
            let Some(value) = data.get(&key) else {
                log::warn!(
                    "{kind} code '{key}' not found in {}, skipping",
                    dat.display()
                );
                continue;
            };
            write_atomic(&path, &to_json(value)?)?;
        }
        result.push((tag, path));
    }
    Ok(result)
}

/// Write sing-box *source* rule-set JSON files (`{"version": 2, "rules": [...]}`) for every
/// requested code into `out_dir` (created if needed), one file per code named `<tag>.json`.
///
/// Files newer than their source .dat are reused as-is. Codes missing from the .dat are
/// skipped (log::warn) and not returned. A .dat path may be `None` only when no codes of
/// that kind are requested. Returns (tag, path) for each file written or reused, geosite
/// first, each in request order.
#[allow(dead_code)]
pub fn build_singbox_rule_sets(
    geosite_dat: Option<&Path>,
    geoip_dat: Option<&Path>,
    geosite_codes: &[String],
    geoip_codes: &[String],
    out_dir: &Path,
) -> Result<Vec<(String, PathBuf)>> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    let mut result = build_kind(
        "geosite",
        geosite_dat,
        geosite_codes,
        geosite_tag,
        out_dir,
        read_geosite,
        geosite_rule_set_json,
    )?;
    result.extend(build_kind(
        "geoip",
        geoip_dat,
        geoip_codes,
        geoip_tag,
        out_dir,
        read_geoip,
        |cidrs: &Vec<String>| geoip_rule_set_json(cidrs),
    )?);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

/// Download `url` to `dest` atomically: write to a unique "<dest>.*.part" then rename. 120s total
/// timeout, follows redirects, fails on non-2xx, and rejects bodies < 1 KiB or HTML
/// (starts with '<') as invalid. Creates parent dirs.
#[allow(dead_code)]
pub async fn download_file(url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    // Unique temp name: a background refresh and a connect-time download of the same
    // file may run at once; each renames a complete file into place.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let part = path_with_suffix(dest, &format!(".{}-{}.part", std::process::id(), nonce));
    let res = match tokio::time::timeout(DOWNLOAD_TIMEOUT, download_to(url, &part)).await {
        Ok(r) => r,
        Err(_) => Err(anyhow!("timed out after {}s", DOWNLOAD_TIMEOUT.as_secs())),
    };
    let res = match res {
        Ok(()) => tokio::fs::rename(&part, dest)
            .await
            .with_context(|| format!("failed to move download into {}", dest.display())),
        Err(e) => Err(e),
    };
    if res.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
    }
    res.with_context(|| format!("failed to download {url}"))
}

async fn download_to(url: &str, part: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("NexVPN/", env!("CARGO_PKG_VERSION")))
        .timeout(DOWNLOAD_TIMEOUT)
        .connect_timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;
    let mut resp = client.get(url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        bail!("HTTP {status}");
    }

    let mut file = tokio::fs::File::create(part)
        .await
        .with_context(|| format!("failed to create {}", part.display()))?;
    let mut total = 0u64;
    let mut head = Vec::with_capacity(4);
    while let Some(chunk) = resp.chunk().await? {
        total += chunk.len() as u64;
        if total > DOWNLOAD_MAX_BYTES {
            bail!("file is larger than {} MiB", DOWNLOAD_MAX_BYTES >> 20);
        }
        if head.len() < 4 {
            let n = (4 - head.len()).min(chunk.len());
            head.extend_from_slice(&chunk[..n]);
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    if total < DOWNLOAD_MIN_BYTES {
        bail!("downloaded file is too small ({total} bytes)");
    }
    // Only the very first byte is checked: protobuf data often starts with "\n" and a
    // length byte, so skipping whitespace would misfire on valid files.
    let body = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&head);
    if body.first() == Some(&b'<') {
        bail!("server returned an HTML page instead of a data file");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::fs::File;

    // --- tiny protobuf encoder -------------------------------------------------

    fn put_varint(out: &mut Vec<u8>, mut v: u64) {
        while v >= 0x80 {
            out.push((v as u8) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }

    fn put_key(out: &mut Vec<u8>, field: u64, wire: u8) {
        put_varint(out, (field << 3) | u64::from(wire));
    }

    fn put_len(out: &mut Vec<u8>, field: u64, data: &[u8]) {
        put_key(out, field, WT_LEN);
        put_varint(out, data.len() as u64);
        out.extend_from_slice(data);
    }

    fn put_uint(out: &mut Vec<u8>, field: u64, v: u64) {
        put_key(out, field, WT_VARINT);
        put_varint(out, v);
    }

    /// Unknown fields of every supported wire type, to check they are skipped.
    fn put_junk(out: &mut Vec<u8>) {
        put_uint(out, 13, u64::MAX); // 10-byte varint
        put_key(out, 14, WT_I64);
        out.extend_from_slice(&[0xAB; 8]);
        put_key(out, 15, WT_I32);
        out.extend_from_slice(&[0xCD; 4]);
        put_len(out, 16, b"\x0a\x03junk");
    }

    const PLAIN: u64 = 0;
    const REGEX: u64 = 1;
    const DOMAIN: u64 = 2;
    const FULL: u64 = 3;

    fn domain(kind: u64, value: &str, attrs: &[&str]) -> Vec<u8> {
        let mut d = Vec::new();
        if kind != 0 {
            put_uint(&mut d, 1, kind); // proto3 omits default values
        }
        put_len(&mut d, 2, value.as_bytes());
        for (i, a) in attrs.iter().enumerate() {
            let mut attr = Vec::new();
            put_len(&mut attr, 1, a.as_bytes());
            if i % 2 == 0 {
                put_uint(&mut attr, 2, 1); // bool_value = true
            } else {
                put_uint(&mut attr, 3, (-1i64) as u64); // int_value = -1
            }
            put_len(&mut d, 3, &attr);
        }
        put_junk(&mut d);
        d
    }

    fn geosite(code: &str, domains: &[Vec<u8>]) -> Vec<u8> {
        let mut e = Vec::new();
        put_len(&mut e, 1, code.as_bytes());
        put_junk(&mut e);
        for d in domains {
            put_len(&mut e, 2, d);
        }
        e
    }

    fn cidr(ip: &[u8], prefix: u64) -> Vec<u8> {
        let mut c = Vec::new();
        put_len(&mut c, 1, ip);
        put_uint(&mut c, 2, prefix);
        c
    }

    fn geoip(code: &str, cidrs: &[Vec<u8>], reverse: bool) -> Vec<u8> {
        let mut e = Vec::new();
        put_len(&mut e, 1, code.as_bytes());
        for c in cidrs {
            put_len(&mut e, 2, c);
        }
        if reverse {
            put_uint(&mut e, 3, 1);
        }
        put_junk(&mut e);
        e
    }

    fn list(entries: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        put_junk(&mut out);
        for e in entries {
            put_len(&mut out, 1, e);
        }
        out
    }

    // --- fixtures ---------------------------------------------------------------

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "nexvpn-geo-test-{}-{name}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn geosite_bytes() -> Vec<u8> {
        list(&[
            geosite("OTHER", &[domain(FULL, "other.example", &[])]),
            geosite(
                "CATEGORY-RU",
                &[
                    domain(FULL, "exact.ru", &[]),
                    domain(DOMAIN, "yandex.ru", &[]),
                    domain(PLAIN, "vk", &[]),
                    domain(REGEX, r"^.+\.gov\.ru$", &[]),
                    domain(DOMAIN, "mail.ru", &[]),
                ],
            ),
            geosite(
                "GOOGLE",
                &[
                    domain(DOMAIN, "google.com", &[]),
                    domain(DOMAIN, "google.cn", &["CN"]),
                    domain(FULL, "ads.google.cn", &["cn", "ads"]),
                    domain(DOMAIN, "doubleclick.net", &["ads"]),
                ],
            ),
            geosite("ONLYCN", &[domain(DOMAIN, "only.cn", &["cn"])]),
            // Duplicate code: first entry wins, like Xray.
            geosite("google", &[domain(DOMAIN, "dup.example", &[])]),
        ])
    }

    fn geoip_bytes() -> Vec<u8> {
        let v6: [u8; 16] = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let v6_host: [u8; 16] = [0x2a, 0x00, 0x1f, 0xa7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        list(&[
            geoip(
                "RU",
                &[
                    cidr(&[1, 2, 3, 4], 24),
                    cidr(&v6, 32),
                    cidr(&v6_host, 29),
                    cidr(&[5, 6, 7, 8], 32),
                    cidr(&[9, 9, 9, 9], 33), // invalid prefix -> skipped
                    cidr(&[1, 2, 3], 8),     // invalid length -> skipped
                ],
                false,
            ),
            geoip(
                "Private",
                &[cidr(&[10, 0, 0, 0], 8), cidr(&[0, 0, 0, 0], 0)],
                false,
            ),
            geoip("NOTCN", &[cidr(&[8, 8, 8, 0], 24)], true),
            geoip("EMPTY", &[], false),
        ])
    }

    fn write_dat(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, data).unwrap();
        p
    }

    fn set_mtime(path: &Path, t: SystemTime) {
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn mtime(path: &Path) -> SystemTime {
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    // --- tests --------------------------------------------------------------------

    #[test]
    fn list_codes_lowercases() {
        let tmp = TempDir::new("list");
        let site = write_dat(&tmp.0, "geosite.dat", &geosite_bytes());
        let ip = write_dat(&tmp.0, "geoip.dat", &geoip_bytes());

        let expected: HashSet<String> = ["other", "category-ru", "google", "onlycn"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(list_codes(&site).unwrap(), expected);

        let expected: HashSet<String> = ["ru", "private", "notcn", "empty"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(list_codes(&ip).unwrap(), expected);

        assert!(list_codes(&tmp.0.join("missing.dat")).is_err());
    }

    #[test]
    fn rejects_malformed_input() {
        let tmp = TempDir::new("bad");
        let html = write_dat(&tmp.0, "html.dat", b"<!DOCTYPE html><html></html>");
        assert!(list_codes(&html).is_err());

        let mut data = geosite_bytes();
        data.truncate(data.len() - 3);
        let truncated = write_dat(&tmp.0, "trunc.dat", &data);
        assert!(list_codes(&truncated).is_err());

        let empty = write_dat(&tmp.0, "empty.dat", b"");
        assert!(list_codes(&empty).unwrap().is_empty());
    }

    #[test]
    fn read_geosite_types_attrs_and_case() {
        let tmp = TempDir::new("site");
        let site = write_dat(&tmp.0, "geosite.dat", &geosite_bytes());
        let codes = strings(&[
            "Category-RU",
            "google",
            "GOOGLE@CN",
            "google@!cn",
            "google@cn@ads",
            "google@",
            "nope",
            "nope@cn",
        ]);
        let got = read_geosite(&site, &codes).unwrap();

        let mut keys: Vec<&str> = got.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "category-ru",
                "google",
                "google@",
                "google@!cn",
                "google@cn",
                "google@cn@ads"
            ]
        );

        assert_eq!(
            got["category-ru"],
            GeoSiteSet {
                full: strings(&["exact.ru"]),
                suffix: strings(&["yandex.ru", "mail.ru"]),
                keyword: strings(&["vk"]),
                regex: strings(&[r"^.+\.gov\.ru$"]),
            }
        );
        // No filter: everything from the first GOOGLE entry (duplicate ignored).
        assert_eq!(
            got["google"],
            GeoSiteSet {
                full: strings(&["ads.google.cn"]),
                suffix: strings(&["google.com", "google.cn", "doubleclick.net"]),
                ..Default::default()
            }
        );
        assert_eq!(got["google@"], got["google"]);
        assert_eq!(
            got["google@cn"],
            GeoSiteSet {
                full: strings(&["ads.google.cn"]),
                suffix: strings(&["google.cn"]),
                ..Default::default()
            }
        );
        assert_eq!(
            got["google@!cn"],
            GeoSiteSet {
                suffix: strings(&["google.com", "doubleclick.net"]),
                ..Default::default()
            }
        );
        assert_eq!(
            got["google@cn@ads"],
            GeoSiteSet {
                full: strings(&["ads.google.cn"]),
                ..Default::default()
            }
        );

        assert!(read_geosite(&site, &[]).unwrap().is_empty());
    }

    #[test]
    fn read_geoip_formats_cidrs() {
        let tmp = TempDir::new("ip");
        let ip = write_dat(&tmp.0, "geoip.dat", &geoip_bytes());
        let got = read_geoip(
            &ip,
            &strings(&["ru", "PRIVATE", "NotCN", "empty", "missing"]),
        )
        .unwrap();

        assert_eq!(got.len(), 4);
        assert_eq!(
            got["ru"],
            strings(&[
                "1.2.3.0/24",
                "2001:db8::/32",
                "2a00:1fa0::/29",
                "5.6.7.8/32"
            ])
        );
        assert_eq!(got["private"], strings(&["10.0.0.0/8", "0.0.0.0/0"]));
        assert_eq!(got["notcn"], strings(&["8.8.8.0/24"])); // reverse_match: as-is
        assert!(got["empty"].is_empty());
        assert!(!got.contains_key("missing"));
    }

    #[test]
    fn tag_naming() {
        assert_eq!(geosite_tag("Category-RU"), "geosite-category-ru");
        assert_eq!(geosite_tag("google@cn"), "geosite-google-at-cn");
        assert_eq!(geosite_tag("google@!CN"), "geosite-google-at-not-cn");
        assert_eq!(geosite_tag("a b/c:d.e_f"), "geosite-a_b_c_d.e_f");
        assert_eq!(geoip_tag("RU"), "geoip-ru");
        assert_eq!(geoip_tag("!cn"), "geoip-not-cn");
    }

    #[test]
    fn build_rule_sets_shape_and_caching() {
        let tmp = TempDir::new("build");
        let site = write_dat(&tmp.0, "geosite.dat", &geosite_bytes());
        let ip = write_dat(&tmp.0, "geoip.dat", &geoip_bytes());
        let past = SystemTime::now() - Duration::from_secs(3600);
        set_mtime(&site, past);
        set_mtime(&ip, past);
        let out = tmp.0.join("rule-sets/nested");

        let site_codes = strings(&[
            "category-ru",
            "GOOGLE@!cn",
            "onlycn@!cn",
            "nope",
            "Category-RU",
        ]);
        let ip_codes = strings(&["RU", "empty", "missing"]);
        let built =
            build_singbox_rule_sets(Some(&site), Some(&ip), &site_codes, &ip_codes, &out).unwrap();

        let tags: Vec<&str> = built.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            tags,
            [
                "geosite-category-ru",
                "geosite-google-at-not-cn",
                "geosite-onlycn-at-not-cn",
                "geoip-ru",
                "geoip-empty"
            ]
        );
        for (tag, path) in &built {
            assert_eq!(path, &out.join(format!("{tag}.json")));
        }
        assert!(!out.join("geosite-nope.json").exists());
        assert!(!out.join("geoip-missing.json").exists());
        let leftovers: Vec<_> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| !n.ends_with(".json"))
            .collect();
        assert!(leftovers.is_empty(), "temp files left: {leftovers:?}");

        assert_eq!(
            read_json(&built[0].1),
            json!({"version": 2, "rules": [{
                "domain": ["exact.ru"],
                "domain_suffix": ["yandex.ru", "mail.ru"],
                "domain_keyword": ["vk"],
                "domain_regex": [r"^.+\.gov\.ru$"],
            }]})
        );
        assert_eq!(
            read_json(&built[1].1),
            json!({"version": 2, "rules": [{"domain_suffix": ["google.com", "doubleclick.net"]}]})
        );
        assert_eq!(
            read_json(&built[2].1),
            json!({"version": 2, "rules": [{"domain": [EMPTY_DOMAIN]}]})
        );
        assert_eq!(
            read_json(&built[3].1),
            json!({"version": 2, "rules": [{"ip_cidr": [
                "1.2.3.0/24", "2001:db8::/32", "2a00:1fa0::/29", "5.6.7.8/32"
            ]}]})
        );
        assert_eq!(
            read_json(&built[4].1),
            json!({"version": 2, "rules": [{"ip_cidr": [EMPTY_CIDR]}]})
        );

        // Second call reuses every file: mtimes unchanged and a marker survives.
        let marker = built[0].1.clone();
        std::fs::write(&marker, b"{\"marker\":true}").unwrap();
        let before: Vec<SystemTime> = built.iter().map(|(_, p)| mtime(p)).collect();
        let again =
            build_singbox_rule_sets(Some(&site), Some(&ip), &site_codes, &ip_codes, &out).unwrap();
        assert_eq!(again, built);
        let after: Vec<SystemTime> = built.iter().map(|(_, p)| mtime(p)).collect();
        assert_eq!(before, after);
        assert_eq!(read_json(&marker), json!({"marker": true}));

        // A newer geosite.dat invalidates only the geosite files.
        set_mtime(&site, SystemTime::now() + Duration::from_secs(3600));
        build_singbox_rule_sets(Some(&site), Some(&ip), &site_codes, &ip_codes, &out).unwrap();
        assert_eq!(read_json(&marker)["version"], json!(2));
        assert_eq!(mtime(&built[3].1), before[3]);
    }

    #[test]
    fn build_rule_sets_requires_dat() {
        let tmp = TempDir::new("req");
        let ip = write_dat(&tmp.0, "geoip.dat", &geoip_bytes());
        let out = tmp.0.join("out");
        let ru = strings(&["ru"]);

        // Only geoip requested: geosite path may be None.
        let built = build_singbox_rule_sets(None, Some(&ip), &[], &ru, &out).unwrap();
        assert_eq!(built.len(), 1);
        // Nothing requested at all: both None is fine.
        assert!(build_singbox_rule_sets(None, None, &[], &[], &out)
            .unwrap()
            .is_empty());

        assert!(build_singbox_rule_sets(None, Some(&ip), &ru, &[], &out).is_err());
        let missing = tmp.0.join("nope/geosite.dat");
        assert!(build_singbox_rule_sets(Some(&missing), Some(&ip), &ru, &[], &out).is_err());
        assert!(build_singbox_rule_sets(None, None, &[], &ru, &out).is_err());
    }
}
