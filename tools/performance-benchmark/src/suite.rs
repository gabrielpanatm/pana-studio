#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Suite {
    Smoke,
    Standard,
    Soak,
}

#[derive(Clone, Debug)]
pub struct SuiteSpec {
    pub label: &'static str,
    pub profiles: &'static [&'static str],
    pub warmup_samples: usize,
    pub kernel_samples: usize,
    pub warm_samples: usize,
    pub cold_samples: usize,
    pub sustained_operations: usize,
    pub resource_poll_ms: u64,
    pub include_bundle_build: bool,
    pub include_ui: bool,
}

impl Suite {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "smoke" => Ok(Self::Smoke),
            "standard" => Ok(Self::Standard),
            "soak" => Ok(Self::Soak),
            _ => Err(format!(
                "Suită necunoscută `{value}`; folosește smoke, standard sau soak."
            )),
        }
    }

    pub const fn spec(self) -> SuiteSpec {
        match self {
            Self::Smoke => SuiteSpec {
                label: "smoke",
                profiles: &["control", "mare"],
                warmup_samples: 2,
                kernel_samples: 20,
                warm_samples: 5,
                cold_samples: 2,
                sustained_operations: 100,
                resource_poll_ms: 100,
                include_bundle_build: true,
                include_ui: true,
            },
            Self::Standard => SuiteSpec {
                label: "standard",
                profiles: &[
                    "control",
                    "mare",
                    "densitate",
                    "margine-disk",
                    "peste-limita",
                ],
                warmup_samples: 5,
                kernel_samples: 100,
                warm_samples: 30,
                cold_samples: 10,
                sustained_operations: 500,
                resource_poll_ms: 100,
                include_bundle_build: true,
                include_ui: true,
            },
            Self::Soak => SuiteSpec {
                label: "soak",
                profiles: &[
                    "control",
                    "mare",
                    "densitate",
                    "margine-disk",
                    "peste-limita",
                ],
                warmup_samples: 10,
                kernel_samples: 250,
                warm_samples: 100,
                cold_samples: 10,
                sustained_operations: 1_500,
                resource_poll_ms: 50,
                include_bundle_build: true,
                include_ui: true,
            },
        }
    }
}
