#[derive(Clone, Copy, PartialEq, Eq, ::prost::Message)]
pub struct Empty {}

#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct Request {
    #[prost(oneof = "request::Call", tags = "3, 5, 6, 7, 8, 11")]
    pub call: ::core::option::Option<request::Call>,
}

pub mod request {
    #[derive(Clone, PartialEq, Eq, ::prost::Message)]
    pub struct GetContractState {
        #[prost(bytes = "bytes", tag = "1")]
        pub address: ::prost::bytes::Bytes,
        #[prost(uint64, optional, tag = "2")]
        pub last_transaction_lt: ::core::option::Option<u64>,
    }

    #[derive(Clone, PartialEq, Eq, ::prost::Message)]
    pub struct GetTransaction {
        #[prost(bytes = "bytes", tag = "1")]
        pub id: ::prost::bytes::Bytes,
    }

    #[derive(Clone, PartialEq, Eq, ::prost::Message)]
    pub struct GetDstTransaction {
        #[prost(bytes = "bytes", tag = "1")]
        pub message_hash: ::prost::bytes::Bytes,
    }

    #[derive(Clone, PartialEq, Eq, ::prost::Message)]
    pub struct SendMessage {
        #[prost(bytes = "bytes", tag = "1")]
        pub message: ::prost::bytes::Bytes,
    }

    #[derive(Clone, PartialEq, Eq, ::prost::Oneof)]
    pub enum Call {
        #[prost(message, tag = "3")]
        GetBlockchainConfig(super::Empty),
        #[prost(message, tag = "5")]
        GetTimings(super::Empty),
        #[prost(message, tag = "6")]
        GetContractState(GetContractState),
        #[prost(message, tag = "7")]
        GetTransaction(GetTransaction),
        #[prost(message, tag = "8")]
        GetDstTransaction(GetDstTransaction),
        #[prost(message, tag = "11")]
        SendMessage(SendMessage),
    }
}

#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct Response {
    #[prost(oneof = "response::Result", tags = "1, 3, 7, 9, 10")]
    pub result: ::core::option::Option<response::Result>,
}

pub mod response {
    #[derive(Clone, PartialEq, Eq, ::prost::Message)]
    pub struct GetRawTransaction {
        #[prost(bytes = "bytes", optional, tag = "1")]
        pub transaction: ::core::option::Option<::prost::bytes::Bytes>,
    }

    #[derive(Clone, PartialEq, Eq, ::prost::Message)]
    pub struct GetTimings {
        #[prost(uint32, tag = "1")]
        pub last_mc_block_seqno: u32,
        #[prost(uint32, tag = "3")]
        pub last_mc_utime: u32,
        #[prost(int64, tag = "4")]
        pub mc_time_diff: i64,
        #[prost(uint64, tag = "6")]
        pub smallest_known_lt: u64,
    }

    #[derive(Clone, PartialEq, Eq, ::prost::Message)]
    pub struct GetBlockchainConfig {
        #[prost(int32, tag = "1")]
        pub global_id: i32,
        #[prost(bytes = "bytes", tag = "2")]
        pub config: ::prost::bytes::Bytes,
        #[prost(uint32, tag = "3")]
        pub seqno: u32,
    }

    #[derive(Clone, PartialEq, Eq, ::prost::Message)]
    pub struct GetContractState {
        #[prost(oneof = "get_contract_state::State", tags = "1, 2, 3")]
        pub state: ::core::option::Option<get_contract_state::State>,
    }

    pub mod get_contract_state {
        #[derive(Clone, Copy, PartialEq, Eq, ::prost::Message)]
        pub struct Timings {
            #[prost(uint64, tag = "1")]
            pub gen_lt: u64,
            #[prost(uint32, tag = "2")]
            pub gen_utime: u32,
        }

        #[derive(Clone, PartialEq, Eq, ::prost::Message)]
        pub struct NotExists {
            #[prost(oneof = "not_exists::GenTimings", tags = "2, 3")]
            pub gen_timings: ::core::option::Option<not_exists::GenTimings>,
        }

        pub mod not_exists {
            #[derive(Clone, PartialEq, Eq, ::prost::Oneof)]
            pub enum GenTimings {
                #[prost(message, tag = "2")]
                Known(super::Timings),
                #[prost(message, tag = "3")]
                Unknown(super::super::super::Empty),
            }
        }

        #[derive(Clone, PartialEq, Eq, ::prost::Message)]
        pub struct Exists {
            #[prost(bytes = "bytes", tag = "1")]
            pub account: ::prost::bytes::Bytes,
            #[prost(message, optional, tag = "2")]
            pub gen_timings: ::core::option::Option<Timings>,
            #[prost(oneof = "exists::LastTransactionId", tags = "3, 4")]
            pub last_transaction_id: ::core::option::Option<exists::LastTransactionId>,
        }

        pub mod exists {
            #[derive(Clone, PartialEq, Eq, ::prost::Message)]
            pub struct Exact {
                #[prost(uint64, tag = "1")]
                pub lt: u64,
                #[prost(bytes = "bytes", tag = "2")]
                pub hash: ::prost::bytes::Bytes,
            }

            #[derive(Clone, Copy, PartialEq, Eq, ::prost::Message)]
            pub struct Inexact {
                #[prost(uint64, tag = "1")]
                pub latest_lt: u64,
            }

            #[derive(Clone, PartialEq, Eq, ::prost::Oneof)]
            pub enum LastTransactionId {
                #[prost(message, tag = "3")]
                Exact(Exact),
                #[prost(message, tag = "4")]
                Inexact(Inexact),
            }
        }

        #[derive(Clone, PartialEq, Eq, ::prost::Oneof)]
        pub enum State {
            #[prost(message, tag = "1")]
            NotExists(NotExists),
            #[prost(message, tag = "2")]
            Exists(Exists),
            #[prost(message, tag = "3")]
            Unchanged(Timings),
        }
    }

    #[derive(Clone, PartialEq, Eq, ::prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        GetRawTransaction(GetRawTransaction),
        #[prost(message, tag = "3")]
        GetTimings(GetTimings),
        #[prost(message, tag = "7")]
        GetBlockchainConfig(GetBlockchainConfig),
        #[prost(message, tag = "9")]
        GetContractState(GetContractState),
        #[prost(message, tag = "10")]
        SendMessage(super::Empty),
    }
}

#[derive(Clone, PartialEq, Eq, ::prost::Message)]
pub struct Error {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
}
