//! ICMP: Internet Control Message Protocol
//!
//! # References
//!
//! - [RFC 792: Internet Control Message Protocol][rfc]
//!
//! [rfc]: https://tools.ietf.org/html/rfc792

use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Range, RangeFrom};

use byteorder::{ByteOrder, NetworkEndian as NE};
use cast::usize;

use fmt::Hex;
use ipv4;
use {Invalid, Resize, Unknown, Valid};

/* Packet structure */
const TYPE: usize = 0;
const CODE: usize = 1;
const CHECKSUM: Range<usize> = 2..4;
const IDENT: Range<usize> = 4..6;
const SEQ_NO: Range<usize> = 6..8;
const PAYLOAD: RangeFrom<usize> = 8..;

/// Size of the ICMP header
pub const HEADER_SIZE: u16 = PAYLOAD.start as u16;

/// ICMP packet
pub struct Packet<BUFFER, TYPE, CHECKSUM>
where
    BUFFER: AsRef<[u8]>,
    TYPE: 'static,
{
    buffer: BUFFER,
    _type: PhantomData<TYPE>,
    _checksum: PhantomData<CHECKSUM>,
}

/// [Type State] The Echo Reply type
pub enum EchoReply {}

/// [Type State] The Echo Request type
pub enum EchoRequest {}

/// [Implementation Detail] EchoReply or EchoRequest
#[doc(hidden)]
pub unsafe trait Echo {}

unsafe impl Echo for EchoReply {}
unsafe impl Echo for EchoRequest {}

/* EchoRequest */
impl<B> Packet<B, EchoRequest, Invalid>
where
    B: AsRef<[u8]> + AsMut<[u8]> + Resize,
{
    /* Constructors */
    /// Transforms the input buffer into a Echo Request ICMP packet
    pub fn new(buffer: B) -> Self {
        assert!(buffer.as_ref().len() >= usize(HEADER_SIZE));

        let mut packet: Packet<B, Unknown, Invalid> = unsafe { Packet::unchecked(buffer) };

        packet.set_type(Type::EchoRequest);
        packet.set_code(0);

        unsafe { Packet::unchecked(packet.buffer) }
    }
}

/* EchoReply OR EchoRequest */
impl<B, E, C> Packet<B, E, C>
where
    B: AsRef<[u8]>,
    E: Echo,
{
    /* Getters */
    /// Returns the Identifier field of the header
    pub fn get_identifier(&self) -> u16 {
        NE::read_u16(&self.as_ref()[IDENT])
    }

    /// Returns the Identifier field of the header
    pub fn get_sequence_number(&self) -> u16 {
        NE::read_u16(&self.as_ref()[SEQ_NO])
    }
}

impl<B, E> Packet<B, E, Invalid>
where
    B: AsRef<[u8]> + AsMut<[u8]>,
    E: Echo,
{
    /* Setters */
    /// Returns the Identifier field of the header
    pub fn set_identifier(&mut self, ident: u16) {
        NE::write_u16(&mut self.as_mut()[IDENT], ident)
    }

    /// Returns the Identifier field of the header
    pub fn set_sequence_number(&mut self, seq_no: u16) {
        NE::write_u16(&mut self.as_mut()[SEQ_NO], seq_no)
    }
}

/* Unknown */
impl<B> Packet<B, Unknown, Valid>
where
    B: AsRef<[u8]> + Resize,
{
    /* Constructors */
    /// Parses the input bytes into a
    pub fn parse(bytes: B) -> Result<Self, B> {
        if bytes.as_ref().len() < usize(HEADER_SIZE) {
            return Err(bytes);
        }

        let packet: Self = unsafe { Packet::unchecked(bytes) };

        if ipv4::verify_checksum(packet.as_bytes()) {
            Ok(packet)
        } else {
            Err(packet.buffer)
        }
    }
}

impl<B> Packet<B, Unknown, Invalid>
where
    B: AsRef<[u8]> + AsMut<[u8]>,
{
    /* Setters */
    /// Sets the Type field of the header
    pub fn set_type(&mut self, type_: Type) {
        self.as_mut()[TYPE] = type_.into();
    }

    /// Sets the Code field of the header
    pub fn set_code(&mut self, code: u8) {
        self.as_mut()[CODE] = code;
    }
}

impl<B> Packet<B, Unknown, Valid>
where
    B: AsRef<[u8]> + AsMut<[u8]>,
{
    /* Setters */
    /// Sets the Type field of the header
    pub fn set_type(self, type_: Type) -> Packet<B, Unknown, Invalid> {
        let mut packet = self.invalidate_header_checksum();
        packet.set_type(type_);
        packet
    }

    /// Sets the Code field of the header
    pub fn set_code(self, code: u8) -> Packet<B, Unknown, Invalid> {
        let mut packet = self.invalidate_header_checksum();
        packet.set_code(code);
        packet
    }
}

impl<B, C> Packet<B, Unknown, C>
where
    B: AsRef<[u8]>,
{
    /// Downcasts this packet with unknown type into a specific type
    pub fn downcast<TYPE>(self) -> Result<Packet<B, TYPE, C>, Self>
    where
        Self: TryInto<Packet<B, TYPE, C>, Error = Self>,
    {
        self.try_into()
    }
}

impl<B, C> From<Packet<B, EchoRequest, C>> for Packet<B, EchoReply, Valid>
where
    B: AsRef<[u8]> + AsMut<[u8]>,
{
    fn from(p: Packet<B, EchoRequest, C>) -> Self {
        let mut p: Packet<B, Unknown, Invalid> = unsafe { Packet::unchecked(p.buffer) };
        p.set_type(Type::EchoReply);
        let p: Packet<B, EchoReply, Invalid> = unsafe { Packet::unchecked(p.buffer) };
        p.update_checksum()
    }
}

impl<B, C> TryFrom<Packet<B, Unknown, C>> for Packet<B, EchoReply, C>
where
    B: AsRef<[u8]>,
{
    type Error = Packet<B, Unknown, C>;

    fn try_from(p: Packet<B, Unknown, C>) -> Result<Self, Packet<B, Unknown, C>> {
        if p.get_type() == Type::EchoReply && p.get_code() == 0 {
            Ok(unsafe { Packet::unchecked(p.buffer) })
        } else {
            Err(p)
        }
    }
}

impl<B, C> TryFrom<Packet<B, Unknown, C>> for Packet<B, EchoRequest, C>
where
    B: AsRef<[u8]>,
{
    type Error = Packet<B, Unknown, C>;

    fn try_from(p: Packet<B, Unknown, C>) -> Result<Self, Packet<B, Unknown, C>> {
        if p.get_type() == Type::EchoRequest && p.get_code() == 0 {
            Ok(unsafe { Packet::unchecked(p.buffer) })
        } else {
            Err(p)
        }
    }
}

/* TYPE */
impl<B, T, C> Packet<B, T, C>
where
    B: AsRef<[u8]>,
{
    /* Constructors */
    unsafe fn unchecked(buffer: B) -> Self {
        Packet {
            buffer,
            _checksum: PhantomData,
            _type: PhantomData,
        }
    }

    /* Getters */
    /// Returns the Type field of the header
    pub fn get_type(&self) -> Type {
        if typeid!(T == EchoReply) {
            Type::EchoReply
        } else if typeid!(T == EchoRequest) {
            Type::EchoRequest
        } else {
            self.as_ref()[TYPE].into()
        }
    }

    /// Returns the Type field of the header
    pub fn get_code(&self) -> u8 {
        if typeid!(T == EchoReply) {
            0
        } else if typeid!(T == EchoRequest) {
            0
        } else {
            self.as_ref()[CODE]
        }
    }

    /// View into the payload
    pub fn payload(&self) -> &[u8] {
        &self.as_ref()[PAYLOAD]
    }

    /// Returns the length (header + data) of this packet
    pub fn len(&self) -> u16 {
        self.as_ref().len() as u16
    }

    /// Returns the byte representation of this packet
    pub fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }

    /* Private */
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }

    fn get_checksum(&self) -> u16 {
        NE::read_u16(&self.as_ref()[CHECKSUM])
    }
}

impl<B, T, C> Packet<B, T, C>
where
    B: AsRef<[u8]> + AsMut<[u8]>,
{
    /* Private */
    fn as_mut(&mut self) -> &mut [u8] {
        self.buffer.as_mut()
    }
}

impl<B, T> Packet<B, T, Invalid>
where
    B: AsRef<[u8]> + AsMut<[u8]>,
{
    /// Mutable view into the payload
    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.as_mut()[PAYLOAD]
    }

    /// Updates the Checksum field of the header
    pub fn update_checksum(mut self) -> Packet<B, T, Valid> {
        let cksum = ipv4::compute_checksum(&self.as_bytes(), CHECKSUM.start);
        NE::write_u16(&mut self.as_mut()[CHECKSUM], cksum);

        unsafe { Packet::unchecked(self.buffer) }
    }
}

impl<B, T> Packet<B, T, Valid>
where
    B: AsRef<[u8]>,
{
    fn invalidate_header_checksum(self) -> Packet<B, T, Invalid> {
        unsafe { Packet::unchecked(self.buffer) }
    }
}

impl<B, T, C> Clone for Packet<B, T, C>
where
    B: AsRef<[u8]> + Clone,
{
    fn clone(&self) -> Self {
        Packet {
            buffer: self.buffer.clone(),
            _type: PhantomData,
            _checksum: PhantomData,
        }
    }
}

/// NOTE excludes the payload
impl<B, E, C> fmt::Debug for Packet<B, E, C>
where
    B: AsRef<[u8]>,
    E: Echo,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("icmp::Packet")
            .field("type", &self.get_type())
            .field("code", &self.get_code())
            .field("checksum", &Hex(self.get_checksum()))
            .field("id", &self.get_identifier())
            .field("seq_no", &self.get_sequence_number())
            // .field("payload", &self.payload())
            .finish()
    }
}

impl<B, C> fmt::Debug for Packet<B, Unknown, C>
where
    B: AsRef<[u8]>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("icmp::Packet")
            .field("type", &self.get_type())
            .field("code", &self.get_code())
            .field("checksum", &Hex(self.get_checksum()))
        // .field("payload", &self.payload())
            .finish()
    }
}

full_range!(u8,
/// ICMP types
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Type {
    /// Echo Reply
    EchoReply = 0,
    /// Destination Unreachable
    DestinationUnreachable = 3,
    /// Echo Request
    EchoRequest = 8,
}
);

#[cfg(test)]
mod tests {
    use rand::{self, Rng};

    use {ether, icmp, mac, ipv4};
    use Buffer;

    const SIZE: usize = 42;

    const BYTES: [u8; SIZE] = [
        255, 255, 255, 255, 255, 255, // eth: destination
        1, 1, 1, 1, 1, 1, // eth: source
        8, 0,  // eth: type
        69, //ipv4: version & ihl
        0, // ipv4: DSCP & ECN
        0, 28, // ipv4: total length
        0, 0, // ipv4: identification
        64, 0, // ipv4: fragments
        64, // ipv4: TTL
        1, // ipv4: protocol
        185, 110, // ipv4: checksum
        192, 168, 0, 33, // ipv4: source
        192, 168, 0, 1, // ipv4: destination
        8, // icmp: type
        0, // icmp: code
        247, 249, // icmp: checksum
        0, 4, // icmp: identifier
        0, 2, // icmp: sequence number
    ];

    const MAC_SRC: mac::Addr = mac::Addr([0x01; 6]);
    const MAC_DST: mac::Addr = mac::Addr([0xff; 6]);

    const IP_SRC: ipv4::Addr = ipv4::Addr([192, 168, 0, 33]);
    const IP_DST: ipv4::Addr = ipv4::Addr([192, 168, 0, 1]);

    #[test]
    fn construct() {
        // NOTE start with randomized array to make sure we set *everything* correctly
        let mut array: [u8; SIZE] = [0; SIZE];
        rand::thread_rng().fill_bytes(&mut array);

        let mut eth = ether::Frame::new(Buffer::new(&mut array));

        eth.set_destination(MAC_DST);
        eth.set_source(MAC_SRC);

        eth.ipv4(|ip| {
            ip.set_destination(IP_DST);
            ip.set_source(IP_SRC);

            ip.echo_request(|icmp| {
                icmp.set_identifier(4);
                icmp.set_sequence_number(2);
            });
        });

        assert_eq!(eth.as_bytes(), &BYTES[..]);
    }

    #[test]
    fn parse() {
        let eth = ether::Frame::parse(&BYTES[..]).unwrap();
        assert_eq!(eth.get_source(), MAC_SRC);
        assert_eq!(eth.get_destination(), MAC_DST);

        let ip = ipv4::Packet::parse(eth.payload()).unwrap();
        assert_eq!(ip.get_destination(), IP_DST);
        assert_eq!(ip.get_source(), IP_SRC);

        let icmp = icmp::Packet::parse(ip.payload())
            .unwrap()
            .downcast::<icmp::EchoRequest>()
            .unwrap();

        assert_eq!(icmp.get_identifier(), 4);
        assert_eq!(icmp.get_sequence_number(), 2);
    }
}
