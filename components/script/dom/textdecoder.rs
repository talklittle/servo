/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::bindings::codegen::Bindings::TextDecoderBinding;
use dom::bindings::codegen::Bindings::TextDecoderBinding::TextDecoderMethods;
use dom::bindings::codegen::UnionTypes::ArrayBufferViewOrArrayBuffer;
use dom::bindings::error::{Error, Fallible};
use dom::bindings::reflector::{Reflector, reflect_dom_object};
use dom::bindings::root::DomRoot;
use dom::bindings::str::{DOMString, USVString};
use dom::globalscope::GlobalScope;
use dom_struct::dom_struct;
use encoding_rs;
use encoding_rs::{Decoder, DecoderResult, Encoding};
use std::borrow::ToOwned;
use std::cell::{Cell, RefCell};

#[dom_struct]
pub struct TextDecoder {
    reflector_: Reflector,
    encoding: &'static Encoding,
    fatal: bool,
    #[ignore_malloc_size_of = "defined in encoding_rs"]
    decoder_: RefCell<Decoder>,
    in_stream_: RefCell<Vec<u8>>,
    do_not_flush_: Cell<bool>,
}

impl TextDecoder {
    fn new_inherited(encoding: &'static Encoding, fatal: bool) -> TextDecoder {
        TextDecoder {
            reflector_: Reflector::new(),
            encoding: encoding,
            fatal: fatal,
            decoder_: RefCell::new(encoding.new_decoder_without_bom_handling()),
            in_stream_: RefCell::new(Vec::new()),
            do_not_flush_: Cell::new(false),
        }
    }

    fn make_range_error() -> Fallible<DomRoot<TextDecoder>> {
        Err(Error::Range("The given encoding is not supported.".to_owned()))
    }

    pub fn new(global: &GlobalScope, encoding: &'static Encoding, fatal: bool) -> DomRoot<TextDecoder> {
        reflect_dom_object(Box::new(TextDecoder::new_inherited(encoding, fatal)),
                           global,
                           TextDecoderBinding::Wrap)
    }

    /// <https://encoding.spec.whatwg.org/#dom-textdecoder>
    pub fn Constructor(global: &GlobalScope,
                       label: DOMString,
                       options: &TextDecoderBinding::TextDecoderOptions)
                            -> Fallible<DomRoot<TextDecoder>> {
        let encoding = match Encoding::for_label_no_replacement(label.as_bytes()) {
            None => return TextDecoder::make_range_error(),
            Some(enc) => enc
        };
        Ok(TextDecoder::new(global, encoding, options.fatal))
    }
}


impl TextDecoderMethods for TextDecoder {
    // https://encoding.spec.whatwg.org/#dom-textdecoder-encoding
    fn Encoding(&self) -> DOMString {
        DOMString::from(self.encoding.name().to_ascii_lowercase())
    }

    // https://encoding.spec.whatwg.org/#dom-textdecoder-fatal
    fn Fatal(&self) -> bool {
        self.fatal
    }

    #[allow(unsafe_code)]
    // https://encoding.spec.whatwg.org/#dom-textdecoder-decode
    fn Decode(&self,
              input: Option<ArrayBufferViewOrArrayBuffer>,
              options: &TextDecoderBinding::TextDecodeOptions)
                    -> Fallible<USVString> {
        if !self.do_not_flush_.get() {
            self.decoder_.replace(self.encoding.new_decoder_without_bom_handling());
            self.in_stream_.replace(Vec::new());
            // TODO unset the "BOM seen flag"
        }

        self.do_not_flush_.set(options.stream);

        match input {
            Some(ArrayBufferViewOrArrayBuffer::ArrayBufferView(mut data)) => {
                self.in_stream_.borrow_mut().extend_from_slice(unsafe { data.as_slice() });
            },
            Some(ArrayBufferViewOrArrayBuffer::ArrayBuffer(mut data)) => {
                self.in_stream_.borrow_mut().extend_from_slice(unsafe { data.as_slice() });
            },
            None => {},
        };

        let mut decoder = self.decoder_.borrow_mut();
        let (remaining, s) = {
            let mut in_stream = self.in_stream_.borrow_mut();

            let (remaining, s) = if self.fatal {
                let mut out_stream = String::with_capacity(
                    decoder.max_utf8_buffer_length_without_replacement(in_stream.len()).unwrap()
                );
                match decoder.decode_to_string_without_replacement(&in_stream, &mut out_stream, !options.stream) {
                    (DecoderResult::InputEmpty, read) => {
                        (in_stream.split_off(read), out_stream)
                    },
                    _ => return Err(Error::Type("Decoding failed".to_owned())),
                }
            } else {
                let valid_up_to = if self.encoding == encoding_rs::UTF_8 {
                    Encoding::utf8_valid_up_to(&in_stream)
                } else if self.encoding == encoding_rs::ISO_2022_JP {
                    Encoding::iso_2022_jp_ascii_valid_up_to(&in_stream)
                } else {
                    Encoding::ascii_valid_up_to(&in_stream)
                };
                let mut out_stream = String::with_capacity(decoder.max_utf8_buffer_length(in_stream.len()).unwrap());
                let (_result, read, _replaced) = decoder.decode_to_string(&in_stream[..valid_up_to], &mut out_stream, !options.stream);
                (in_stream.split_off(read), out_stream)
            };
            (remaining, s)
        };
        self.in_stream_.replace(remaining);
        Ok(USVString(s))
    }
}
