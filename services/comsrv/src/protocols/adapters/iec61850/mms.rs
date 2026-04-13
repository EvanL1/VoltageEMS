//! MMS (Manufacturing Message Specification) PDU encoder and decoder.
//!
//! Implements a minimal subset of MMS (ISO 9506) required for IEC 61850 data collection:
//!
//! - **Read-Request**: read a single domain-specific variable
//! - **Read-Response**: parse the returned `AccessResult`
//! - **Write-Request**: write a single variable (for control)
//! - **Initiate-Request/Response**: already handled as static bytes in `transport.rs`
//!
//! # Wire format (MMS Read)
//!
//! ```text
//! A0 [len]          confirmedRequestPDU
//!   02 01 [id]      invokeID
//!   A4 [len]        Read-Request [4]
//!     A1 [len]      variableAccessSpec (list scope)
//!       A0 [len]    VariableSpecification: name [0]
//!         30 [len]  SEQUENCE (ObjectName wrapper)
//!           A0 [len]  outer name [0]
//!             A1 [len]  domain-specific [1]
//!               1A [dlen] [domain]   VisibleString: domain id
//!               1A [ilen] [item]     VisibleString: item id
//! ```
//!
//! The nesting matches captured libiec61850 traffic; no C code has been copied.

use crate::protocols::core::error::{GatewayError, Result};

use super::transport::{parse_ber_len, parse_tlv, push_ber_len};

// ── MMS tag constants ─────────────────────────────────────────────────────────

const TAG_CONFIRMED_REQ: u8 = 0xA0;
const TAG_CONFIRMED_RESP: u8 = 0xA1;
const TAG_REJECT_PDU: u8 = 0xA4; // [4] = both Read tag and Reject (context-dependent)
const TAG_INTEGER: u8 = 0x02;
const TAG_VISIBLE_STR: u8 = 0x1A;
const TAG_SEQUENCE: u8 = 0x30;

// AccessResult Data type tags (context-specific primitive)
const DATA_BOOLEAN: u8 = 0x83;
const DATA_BIT_STRING: u8 = 0x84;
const DATA_INTEGER: u8 = 0x85;
const DATA_UNSIGNED: u8 = 0x86;
const DATA_FLOAT: u8 = 0x87;
const DATA_OCTET_STRING: u8 = 0x89;
const DATA_VISIBLE_STRING: u8 = 0x8A;
const DATA_UTC_TIME: u8 = 0x91;
const DATA_MMS_STRING: u8 = 0x90;

// ── Public API ────────────────────────────────────────────────────────────────

/// A value decoded from an MMS AccessResult.
#[derive(Debug, Clone)]
pub enum MmsValue {
    /// IEEE 754 single precision (5-byte MMS float: 1 exponent + 4 data)
    Float32(f32),
    /// IEEE 754 double precision (9-byte MMS float: 1 exponent + 8 data)
    Float64(f64),
    Boolean(bool),
    Integer(i64),
    Unsigned(u64),
    VisibleString(String),
    BitString {
        bytes: Vec<u8>,
        unused_bits: u8,
    },
    UtcTime([u8; 8]),
    OctetString(Vec<u8>),
    /// MMS data-access-error code
    Failure(u8),
}

impl MmsValue {
    /// Convert to `f64` for use in DataPoint.
    pub fn to_f64(&self) -> Option<f64> {
        match self {
            Self::Float32(v) => Some(*v as f64),
            Self::Float64(v) => Some(*v),
            Self::Integer(v) => Some(*v as f64),
            Self::Unsigned(v) => Some(*v as f64),
            Self::Boolean(v) => Some(if *v { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn to_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(v) => Some(*v),
            Self::Integer(v) => Some(*v != 0),
            Self::Unsigned(v) => Some(*v != 0),
            _ => None,
        }
    }

    pub fn to_string_val(&self) -> Option<String> {
        match self {
            Self::VisibleString(s) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn is_ok(&self) -> bool {
        !matches!(self, Self::Failure(_))
    }
}

// ── Encoder ───────────────────────────────────────────────────────────────────

/// Build an MMS Read-Request PDU for a single domain-specific variable.
///
/// `invoke_id`: 1–255, used to match request with response.
/// `domain`: MMS domain (IED logical device name), e.g. `"simpleIOGenericIO"`.
/// `item`: MMS item ID with functional constraint, e.g. `"GGIO1$MX$AnIn1$mag$f"`.
pub fn build_read_request(invoke_id: u8, domain: &str, item: &str) -> Vec<u8> {
    let d = domain.as_bytes();
    let i = item.as_bytes();

    // Sizes are computed bottom-up. Each variable = CONTENT length of that TLV
    // (i.e. what goes into push_ber_len for that tag). Total TLV size = 2 + content.
    let a1_domain_content = 2 + d.len() + 2 + i.len(); // 1A dl [d] 1A il [i]
    let a0_outer_content = 2 + a1_domain_content; // A1 TLV total
    let seq_content = 2 + a0_outer_content; // A0 TLV total
    let a0_varspec_content = 2 + seq_content; // 30 TLV total
    let a1_list_content = 2 + a0_varspec_content; // A0 TLV total
    let a4_read_content = 2 + a1_list_content; // A1 TLV total
                                               // outer A0 content = invokeID(3) + A4 TLV total(2 + a4_read_content)
    let req_inner = 3 + 2 + a4_read_content;

    let mut buf = Vec::with_capacity(2 + req_inner);

    // confirmedRequestPDU A0 [req_inner]
    buf.push(TAG_CONFIRMED_REQ);
    push_ber_len(&mut buf, req_inner);

    // invokeID: 02 01 [id]
    buf.extend_from_slice(&[TAG_INTEGER, 0x01, invoke_id]);

    // Read [4]: A4, content = a4_read_content
    buf.push(TAG_REJECT_PDU); // 0xA4 = [4] IMPLICIT = Read
    push_ber_len(&mut buf, a4_read_content);

    // A1, content = a1_list_content
    buf.push(0xA1);
    push_ber_len(&mut buf, a1_list_content);

    // A0, content = a0_varspec_content
    buf.push(0xA0);
    push_ber_len(&mut buf, a0_varspec_content);

    // 30 (ObjectName SEQUENCE), content = seq_content
    buf.push(TAG_SEQUENCE);
    push_ber_len(&mut buf, seq_content);

    // A0, content = a0_outer_content
    buf.push(0xA0);
    push_ber_len(&mut buf, a0_outer_content);

    // A1 (domain-specific), content = a1_domain_content
    buf.push(0xA1);
    push_ber_len(&mut buf, a1_domain_content);

    // 1A [domain_len] [domain]
    buf.push(TAG_VISIBLE_STR);
    buf.push(d.len() as u8);
    buf.extend_from_slice(d);

    // 1A [item_len] [item]
    buf.push(TAG_VISIBLE_STR);
    buf.push(i.len() as u8);
    buf.extend_from_slice(i);

    buf
}

// ── Decoder ───────────────────────────────────────────────────────────────────

/// Decode an MMS Read-Response and return the first `AccessResult` value.
///
/// Returns `Err` if the PDU is malformed.  Returns `Ok(MmsValue::Failure(n))` if
/// the server returned a data-access error.
pub fn parse_read_response(pdu: &[u8]) -> Result<(u8, MmsValue)> {
    // A1 [len]  confirmedResponsePDU — parse into its CONTENT, not the bytes after it
    let (_, pdu_content) = parse_tlv(pdu, TAG_CONFIRMED_RESP)
        .ok_or_else(|| GatewayError::Protocol("MMS: expected confirmedResponsePDU (A1)".into()))?;

    // 02 01 [id]  invokeID
    let (rest, id_bytes) = parse_tlv(pdu_content, TAG_INTEGER)
        .ok_or_else(|| GatewayError::Protocol("MMS: missing invokeID".into()))?;
    let invoke_id = id_bytes.first().copied().unwrap_or(0);

    // A4 [len]  Read-Response [4]
    let (_, read_resp) = parse_tlv(rest, TAG_REJECT_PDU)
        .ok_or_else(|| GatewayError::Protocol("MMS: expected Read-Response (A4)".into()))?;

    // A1 [len]  listOfAccessResults
    let (_, access_results) = parse_tlv(read_resp, 0xA1)
        .ok_or_else(|| GatewayError::Protocol("MMS: expected listOfAccessResults (A1)".into()))?;

    // First AccessResult: either A1 (success wrapper) or direct primitive tag
    let value = parse_access_result(access_results)?;

    Ok((invoke_id, value))
}

/// Parse a single AccessResult from the start of `buf`.
fn parse_access_result(buf: &[u8]) -> Result<MmsValue> {
    if buf.is_empty() {
        return Err(GatewayError::Protocol("MMS: empty AccessResult".into()));
    }

    let tag = buf[0];

    // Failure: [0] = A0 with inner = [0] failure code
    // In MMS, AccessResult CHOICE:
    //   failure [0] IMPLICIT DataAccessError   → tag = 0x80 (primitive)
    //   success [1] IMPLICIT Data              → tag = 0xA1 (constructed)
    // But many servers return data directly (primitive tags), so handle both.

    match tag {
        0x80 => {
            // failure (DataAccessError)
            let code = buf.get(2).copied().unwrap_or(0);
            Ok(MmsValue::Failure(code))
        },
        0xA1 => {
            // success: A1 contains a Data CHOICE value
            let (_, data_buf) = parse_tlv(buf, 0xA1).ok_or_else(|| {
                GatewayError::Protocol("MMS: malformed AccessResult success".into())
            })?;
            parse_data(data_buf)
        },
        // Direct data tags (some servers embed data without the A1 wrapper)
        DATA_BOOLEAN | DATA_BIT_STRING | DATA_INTEGER | DATA_UNSIGNED | DATA_FLOAT
        | DATA_OCTET_STRING | DATA_VISIBLE_STRING | DATA_UTC_TIME | DATA_MMS_STRING => {
            parse_data(buf)
        },
        other => Err(GatewayError::Protocol(format!(
            "MMS: unknown AccessResult tag 0x{:02X}",
            other
        ))),
    }
}

/// Parse a single MMS `Data` value (the inner content of AccessResult success).
fn parse_data(buf: &[u8]) -> Result<MmsValue> {
    if buf.is_empty() {
        return Err(GatewayError::Protocol("MMS: empty Data".into()));
    }

    let tag = buf[0];
    let (len, hdr) = parse_ber_len(&buf[1..])
        .ok_or_else(|| GatewayError::Protocol("MMS: BER length error".into()))?;
    let val = &buf[1 + hdr..1 + hdr + len];

    match tag {
        DATA_BOOLEAN => Ok(MmsValue::Boolean(val.first().copied().unwrap_or(0) != 0)),

        DATA_INTEGER => {
            if val.is_empty() {
                return Err(GatewayError::Protocol("MMS: zero-length integer".into()));
            }
            let mut n: i64 = if val[0] & 0x80 != 0 { -1i64 } else { 0 };
            for &b in val {
                n = (n << 8) | (b as i64);
            }
            Ok(MmsValue::Integer(n))
        },

        DATA_UNSIGNED => {
            let mut n: u64 = 0;
            for &b in val {
                n = (n << 8) | (b as u64);
            }
            Ok(MmsValue::Unsigned(n))
        },

        DATA_FLOAT => {
            if val.len() == 5 {
                // float32: [exponent_bits(1)] [ieee754_be(4)]
                let bytes = [val[1], val[2], val[3], val[4]];
                let f = f32::from_be_bytes(bytes);
                Ok(MmsValue::Float32(f))
            } else if val.len() == 9 {
                // float64: [exponent_bits(1)] [ieee754_be(8)]
                let bytes = [
                    val[1], val[2], val[3], val[4], val[5], val[6], val[7], val[8],
                ];
                let f = f64::from_be_bytes(bytes);
                Ok(MmsValue::Float64(f))
            } else {
                Err(GatewayError::Protocol(format!(
                    "MMS: unexpected float length {}",
                    val.len()
                )))
            }
        },

        DATA_BIT_STRING => {
            let unused = val.first().copied().unwrap_or(0);
            Ok(MmsValue::BitString {
                bytes: val[1..].to_vec(),
                unused_bits: unused,
            })
        },

        DATA_OCTET_STRING => Ok(MmsValue::OctetString(val.to_vec())),

        DATA_VISIBLE_STRING | DATA_MMS_STRING => {
            let s = String::from_utf8_lossy(val).into_owned();
            Ok(MmsValue::VisibleString(s))
        },

        DATA_UTC_TIME => {
            if val.len() == 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(val);
                Ok(MmsValue::UtcTime(arr))
            } else {
                Err(GatewayError::Protocol(format!(
                    "MMS: UTC time length {} (expected 8)",
                    val.len()
                )))
            }
        },

        other => Err(GatewayError::Protocol(format!(
            "MMS: unknown Data tag 0x{:02X}",
            other
        ))),
    }
}

// ── Write-Request ─────────────────────────────────────────────────────────────

/// Build an MMS Write-Request to set a `boolean` value.
pub fn build_write_bool_request(invoke_id: u8, domain: &str, item: &str, value: bool) -> Vec<u8> {
    build_write_request(invoke_id, domain, item, &[DATA_BOOLEAN, 0x01, value as u8])
}

/// Build an MMS Write-Request to set a `float32` value.
pub fn build_write_f32_request(invoke_id: u8, domain: &str, item: &str, value: f32) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    build_write_request(
        invoke_id,
        domain,
        item,
        &[
            DATA_FLOAT, 0x05, 0x08, bytes[0], bytes[1], bytes[2], bytes[3],
        ],
    )
}

/// Generic Write-Request builder. `data_bytes` = the encoded Data TLV to write.
fn build_write_request(invoke_id: u8, domain: &str, item: &str, data_bytes: &[u8]) -> Vec<u8> {
    let d = domain.as_bytes();
    let i = item.as_bytes();

    let ds_inner = 2 + d.len() + 2 + i.len();
    let name_inner = 2 + ds_inner;
    let outer_name_inner = 2 + name_inner;
    let seq_inner = 2 + outer_name_inner;
    let var_spec_inner = 2 + seq_inner;
    let list_inner = 2 + var_spec_inner;

    // Write-Request [5]: A5
    // A0 [listOfVariable]: same structure as read
    // A1 [listOfData]: data value(s)
    let data_list_inner = data_bytes.len();
    let write_inner = 2 + list_inner + 2 + data_list_inner;
    let req_inner = 3 + 2 + write_inner;

    let mut buf = Vec::with_capacity(2 + req_inner);

    buf.push(TAG_CONFIRMED_REQ);
    push_ber_len(&mut buf, req_inner);
    buf.extend_from_slice(&[TAG_INTEGER, 0x01, invoke_id]);

    // Write [5]: A5
    buf.push(0xA5);
    push_ber_len(&mut buf, write_inner);

    // listOfVariable: A0
    buf.push(0xA0);
    push_ber_len(&mut buf, list_inner);
    buf.push(0xA1);
    push_ber_len(&mut buf, var_spec_inner);
    buf.push(0xA0);
    push_ber_len(&mut buf, var_spec_inner - 2);
    buf.push(TAG_SEQUENCE);
    push_ber_len(&mut buf, seq_inner - 2);
    buf.push(0xA0);
    push_ber_len(&mut buf, outer_name_inner - 2);
    buf.push(0xA1);
    push_ber_len(&mut buf, ds_inner);
    buf.push(TAG_VISIBLE_STR);
    buf.push(d.len() as u8);
    buf.extend_from_slice(d);
    buf.push(TAG_VISIBLE_STR);
    buf.push(i.len() as u8);
    buf.extend_from_slice(i);

    // listOfData: A1
    buf.push(0xA1);
    push_ber_len(&mut buf, data_list_inner);
    buf.extend_from_slice(data_bytes);

    buf
}

/// Parse an MMS Write-Response. Returns `Ok(invoke_id)` on success.
pub fn parse_write_response(pdu: &[u8]) -> Result<u8> {
    let (rest, _) = parse_tlv(pdu, TAG_CONFIRMED_RESP)
        .ok_or_else(|| GatewayError::Protocol("MMS: expected confirmedResponsePDU (A1)".into()))?;
    let (_, id_bytes) = parse_tlv(rest, TAG_INTEGER)
        .ok_or_else(|| GatewayError::Protocol("MMS: missing invokeID in write response".into()))?;
    Ok(id_bytes.first().copied().unwrap_or(0))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_request_matches_capture() {
        // Captured from libiec61850 client reading simpleIOGenericIO/GGIO1.AnIn1.mag.f
        let expected = hex::decode(
            "a038020101a433a131a02f302da02ba1291a1173696d706c65494f47656e65\
             726963494f1a144747494f31244d5824416e496e31246d61672466",
        )
        .unwrap();

        let got = build_read_request(1, "simpleIOGenericIO", "GGIO1$MX$AnIn1$mag$f");
        assert_eq!(got, expected, "Read request bytes mismatch");
    }

    #[test]
    fn parse_float_response() {
        // MMS Read-Response for a float32 value ≈ -0.0977 (0xBDC6AF00):
        //   A1 10  confirmedResponsePDU (content=16)
        //     02 01 01  invokeID=1
        //     A4 0B  Read-Response (content=11)
        //       A1 09  listOfAccessResults (content=9)
        //         A1 07  AccessResult success (content=7)
        //           87 05  Data floating-point (content=5)
        //             08 BD C6 AF 00  (exponent=8, IEEE-754 = -0.0977)
        let resp = hex::decode("a110020101a40ba109a107870508bdc6af00").unwrap();
        let (id, val) = parse_read_response(&resp).unwrap();
        assert_eq!(id, 1);
        match val {
            MmsValue::Float32(f) => {
                // 0xBDC6AF00 ≈ -0.0977
                assert!(
                    (f - (-0.0977f32)).abs() < 0.001,
                    "float value mismatch: {}",
                    f
                );
            },
            other => panic!("expected Float32, got {:?}", other),
        }
    }
}
