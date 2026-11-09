//! D-Bus message encoding, shared by every opt-in desktop service.
#[cfg(target_os = "linux")]
use super::super::a11y_bus::{pad, put_str};
#[cfg(not(target_os = "linux"))]
use super::a11y_bus::{pad, put_str};
pub(super) fn encode(
    kind: u8,
    serial: u32,
    fields: Vec<(u8, u8, &str)>,
    reply: Option<u32>,
    body: Vec<u8>,
) -> Vec<u8> {
    let mut out = vec![b'l', kind, 0, 1];
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&serial.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    for (code, sig, value) in fields {
        pad(&mut out, 0, 8);
        out.extend_from_slice(&[code, 1, sig, 0]);
        if sig == b'g' {
            out.push(value.len() as u8);
            out.extend_from_slice(value.as_bytes());
            out.push(0);
        } else {
            put_str(&mut out, value);
        }
    }
    if let Some(reply) = reply {
        pad(&mut out, 0, 8);
        out.extend_from_slice(&[5, 1, b'u', 0]);
        out.extend_from_slice(&reply.to_le_bytes());
    }
    let len = (out.len() - 16) as u32;
    out[12..16].copy_from_slice(&len.to_le_bytes());
    pad(&mut out, 0, 8);
    out.extend(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::{
        Message,
        zvariant::{
            Endian,
            serialized::{Context, Data},
        },
    };
    fn parse(bytes: Vec<u8>) -> Message {
        // SAFETY: our wire codec never sends file descriptors.
        unsafe { Message::from_bytes(Data::new(bytes, Context::new_dbus(Endian::Little, 0))) }
            .unwrap()
    }
    #[test]
    fn calls_preserve_destination_signature_body_and_serial() {
        let mut body = Vec::new();
        put_str(&mut body, "stillkit");
        let m = parse(encode(
            1,
            42,
            vec![
                (1, b'o', "/org/test"),
                (2, b's', "org.test.Service"),
                (3, b's', "Notify"),
                (6, b's', "org.test.Service"),
                (8, b'g', "s"),
            ],
            None,
            body,
        ));
        assert_eq!(
            m.header().destination().unwrap().as_str(),
            "org.test.Service"
        );
        assert_eq!(m.header().member().unwrap().as_str(), "Notify");
        assert_eq!(m.body().deserialize::<&str>().unwrap(), "stillkit");
        assert_eq!(m.primary_header().serial_num().get(), 42);
    }
    #[test]
    fn replies_carry_the_call_serial_and_empty_body() {
        let m = parse(encode(
            2,
            7,
            vec![(6, b's', ":1.12"), (8, b'g', "")],
            Some(42),
            vec![],
        ));
        assert_eq!(m.header().reply_serial().unwrap().get(), 42);
        m.body().deserialize::<()>().unwrap();
    }
    #[test]
    fn tooltip_change_is_a_signal_without_a_reply() {
        let m = parse(encode(
            4,
            8,
            vec![
                (1, b'o', "/StatusNotifierItem"),
                (2, b's', "org.kde.StatusNotifierItem"),
                (3, b's', "NewToolTip"),
            ],
            None,
            vec![],
        ));
        assert_eq!(m.message_type(), zbus::message::Type::Signal);
        assert_eq!(m.header().member().unwrap().as_str(), "NewToolTip");
    }
}
