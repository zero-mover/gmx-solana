export type Chainlink = {
  "version": "1.0.0",
  "name": "store",
  "constants": [
    {
      "name": "HEADER_SIZE",
      "type": {
        "defined": "usize"
      },
      "value": "192"
    }
  ],
  "instructions": [
    {
      "name": "createFeed",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "description",
          "type": "string"
        },
        {
          "name": "decimals",
          "type": "u8"
        },
        {
          "name": "granularity",
          "type": "u8"
        },
        {
          "name": "liveLength",
          "type": "u32"
        }
      ]
    },
    {
      "name": "closeFeed",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "receiver",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": []
    },
    {
      "name": "transferFeedOwnership",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "proposedOwner",
          "type": "publicKey"
        }
      ]
    },
    {
      "name": "acceptFeedOwnership",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "proposedOwner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": []
    },
    {
      "name": "setValidatorConfig",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "flaggingThreshold",
          "type": "u32"
        }
      ]
    },
    {
      "name": "setWriter",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "writer",
          "type": "publicKey"
        }
      ]
    },
    {
      "name": "lowerFlag",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        },
        {
          "name": "accessController",
          "isMut": false,
          "isSigner": false
        }
      ],
      "args": []
    },
    {
      "name": "submit",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "round",
          "type": {
            "defined": "NewTransmission"
          }
        }
      ]
    },
    {
      "name": "initialize",
      "accounts": [
        {
          "name": "store",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": true
        },
        {
          "name": "loweringAccessController",
          "isMut": false,
          "isSigner": false
        }
      ],
      "args": []
    },
    {
      "name": "transferStoreOwnership",
      "accounts": [
        {
          "name": "store",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "proposedOwner",
          "type": "publicKey"
        }
      ]
    },
    {
      "name": "acceptStoreOwnership",
      "accounts": [
        {
          "name": "store",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": []
    },
    {
      "name": "setLoweringAccessController",
      "accounts": [
        {
          "name": "store",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        },
        {
          "name": "accessController",
          "isMut": false,
          "isSigner": false
        }
      ],
      "args": []
    },
    {
      "name": "query",
      "accounts": [
        {
          "name": "feed",
          "isMut": false,
          "isSigner": false
        }
      ],
      "args": [
        {
          "name": "scope",
          "type": {
            "defined": "Scope"
          }
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "Store",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "publicKey"
          },
          {
            "name": "proposedOwner",
            "type": "publicKey"
          },
          {
            "name": "loweringAccessController",
            "type": "publicKey"
          }
        ]
      }
    },
    {
      "name": "Transmissions",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "type": "u8"
          },
          {
            "name": "state",
            "type": "u8"
          },
          {
            "name": "owner",
            "type": "publicKey"
          },
          {
            "name": "proposedOwner",
            "type": "publicKey"
          },
          {
            "name": "writer",
            "type": "publicKey"
          },
          {
            "name": "description",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "decimals",
            "type": "u8"
          },
          {
            "name": "flaggingThreshold",
            "type": "u32"
          },
          {
            "name": "latestRoundId",
            "type": "u32"
          },
          {
            "name": "granularity",
            "type": "u8"
          },
          {
            "name": "liveLength",
            "type": "u32"
          },
          {
            "name": "liveCursor",
            "type": "u32"
          },
          {
            "name": "historicalCursor",
            "type": "u32"
          }
        ]
      }
    }
  ],
  "types": [
    {
      "name": "NewTransmission",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "timestamp",
            "type": "u64"
          },
          {
            "name": "answer",
            "type": "i128"
          }
        ]
      }
    },
    {
      "name": "Round",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "roundId",
            "type": "u32"
          },
          {
            "name": "slot",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "u32"
          },
          {
            "name": "answer",
            "type": "i128"
          }
        ]
      }
    },
    {
      "name": "Scope",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "Version"
          },
          {
            "name": "Decimals"
          },
          {
            "name": "Description"
          },
          {
            "name": "RoundData",
            "fields": [
              {
                "name": "round_id",
                "type": "u32"
              }
            ]
          },
          {
            "name": "LatestRoundData"
          },
          {
            "name": "Aggregator"
          }
        ]
      }
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "Unauthorized",
      "msg": "Unauthorized"
    },
    {
      "code": 6001,
      "name": "InvalidInput",
      "msg": "Invalid input"
    },
    {
      "code": 6002,
      "name": "NotFound"
    },
    {
      "code": 6003,
      "name": "InvalidVersion",
      "msg": "Invalid version"
    },
    {
      "code": 6004,
      "name": "InsufficientSize",
      "msg": "Insufficient or invalid feed account size, has to be `8 + HEADER_SIZE + n * size_of::<Transmission>()`"
    }
  ]
}

export const IDL: Chainlink = {
  "version": "1.0.0",
  "name": "store",
  "constants": [
    {
      "name": "HEADER_SIZE",
      "type": {
        "defined": "usize"
      },
      "value": "192"
    }
  ],
  "instructions": [
    {
      "name": "createFeed",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "description",
          "type": "string"
        },
        {
          "name": "decimals",
          "type": "u8"
        },
        {
          "name": "granularity",
          "type": "u8"
        },
        {
          "name": "liveLength",
          "type": "u32"
        }
      ]
    },
    {
      "name": "closeFeed",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "receiver",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": []
    },
    {
      "name": "transferFeedOwnership",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "proposedOwner",
          "type": "publicKey"
        }
      ]
    },
    {
      "name": "acceptFeedOwnership",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "proposedOwner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": []
    },
    {
      "name": "setValidatorConfig",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "flaggingThreshold",
          "type": "u32"
        }
      ]
    },
    {
      "name": "setWriter",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "writer",
          "type": "publicKey"
        }
      ]
    },
    {
      "name": "lowerFlag",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        },
        {
          "name": "accessController",
          "isMut": false,
          "isSigner": false
        }
      ],
      "args": []
    },
    {
      "name": "submit",
      "accounts": [
        {
          "name": "feed",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "round",
          "type": {
            "defined": "NewTransmission"
          }
        }
      ]
    },
    {
      "name": "initialize",
      "accounts": [
        {
          "name": "store",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "owner",
          "isMut": false,
          "isSigner": true
        },
        {
          "name": "loweringAccessController",
          "isMut": false,
          "isSigner": false
        }
      ],
      "args": []
    },
    {
      "name": "transferStoreOwnership",
      "accounts": [
        {
          "name": "store",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": [
        {
          "name": "proposedOwner",
          "type": "publicKey"
        }
      ]
    },
    {
      "name": "acceptStoreOwnership",
      "accounts": [
        {
          "name": "store",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        }
      ],
      "args": []
    },
    {
      "name": "setLoweringAccessController",
      "accounts": [
        {
          "name": "store",
          "isMut": true,
          "isSigner": false
        },
        {
          "name": "authority",
          "isMut": false,
          "isSigner": true
        },
        {
          "name": "accessController",
          "isMut": false,
          "isSigner": false
        }
      ],
      "args": []
    },
    {
      "name": "query",
      "accounts": [
        {
          "name": "feed",
          "isMut": false,
          "isSigner": false
        }
      ],
      "args": [
        {
          "name": "scope",
          "type": {
            "defined": "Scope"
          }
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "Store",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "publicKey"
          },
          {
            "name": "proposedOwner",
            "type": "publicKey"
          },
          {
            "name": "loweringAccessController",
            "type": "publicKey"
          }
        ]
      }
    },
    {
      "name": "Transmissions",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "type": "u8"
          },
          {
            "name": "state",
            "type": "u8"
          },
          {
            "name": "owner",
            "type": "publicKey"
          },
          {
            "name": "proposedOwner",
            "type": "publicKey"
          },
          {
            "name": "writer",
            "type": "publicKey"
          },
          {
            "name": "description",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "decimals",
            "type": "u8"
          },
          {
            "name": "flaggingThreshold",
            "type": "u32"
          },
          {
            "name": "latestRoundId",
            "type": "u32"
          },
          {
            "name": "granularity",
            "type": "u8"
          },
          {
            "name": "liveLength",
            "type": "u32"
          },
          {
            "name": "liveCursor",
            "type": "u32"
          },
          {
            "name": "historicalCursor",
            "type": "u32"
          }
        ]
      }
    }
  ],
  "types": [
    {
      "name": "NewTransmission",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "timestamp",
            "type": "u64"
          },
          {
            "name": "answer",
            "type": "i128"
          }
        ]
      }
    },
    {
      "name": "Round",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "roundId",
            "type": "u32"
          },
          {
            "name": "slot",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "u32"
          },
          {
            "name": "answer",
            "type": "i128"
          }
        ]
      }
    },
    {
      "name": "Scope",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "Version"
          },
          {
            "name": "Decimals"
          },
          {
            "name": "Description"
          },
          {
            "name": "RoundData",
            "fields": [
              {
                "name": "round_id",
                "type": "u32"
              }
            ]
          },
          {
            "name": "LatestRoundData"
          },
          {
            "name": "Aggregator"
          }
        ]
      }
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "Unauthorized",
      "msg": "Unauthorized"
    },
    {
      "code": 6001,
      "name": "InvalidInput",
      "msg": "Invalid input"
    },
    {
      "code": 6002,
      "name": "NotFound"
    },
    {
      "code": 6003,
      "name": "InvalidVersion",
      "msg": "Invalid version"
    },
    {
      "code": 6004,
      "name": "InsufficientSize",
      "msg": "Insufficient or invalid feed account size, has to be `8 + HEADER_SIZE + n * size_of::<Transmission>()`"
    }
  ]
}
