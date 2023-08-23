#[macro_export]
macro_rules! item_definition {
    (
        $type_name:ident {
            $(
                $name: ident($($item_args:tt)+)
            ),+
        }
    ) => {
        $crate::item_helper!(@enum $type_name {
            $(
                $name($($item_args)+)
            ),+
        });

        impl $type_name {
            pub fn display_name(&self) -> &str {
                match self {
                    $(
                        $type_name::$name => $crate::item_helper!(@display_name $name($($item_args)+))
                    ),+
                }
            }

            pub fn is_extractable(&self) -> bool {
                match self {
                    $(
                        $type_name::$name => $crate::item_helper!(@extractable $name($($item_args)+))
                    ),+
                }
            }

            pub fn is_fluid(&self) -> bool {
                match self {
                    $(
                        $type_name::$name => $crate::item_helper!(@fluid $name($($item_args)+))
                    ),+
                }
            }

            pub fn sink_points(&self) -> Option<u32> {
                match self {
                    $(
                        $type_name::$name => $crate::item_helper!(@sink_points $name($($item_args)+))
                    ),+
                }
            }

            pub fn from_str(value: &str) -> Option<$type_name> where Self: Sized {
                match value {
                    $(
                        $crate::item_helper!(@display_name $name($($item_args)+)) => Some($type_name::$name)
                    ),+,
                    _ => None
                }
            }
        }

        impl fmt::Display for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.display_name())
            }
        }
    };
}
#[macro_export]
macro_rules! item_helper {
    (
        @enum $type_name:ident {
            $(
                $name: ident(name: $display_name:literal $($machine_args:tt)*)
            ),+
        }
    ) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        pub enum $type_name {
            $(
                #[serde(rename = $display_name)]
                $name
            ),+
        }
    };
    // Display Name
    (
        @display_name $name: ident()
    ) => {
        compile_error!("Missing name attribute for the item {}", $name);
    };

    (
        @display_name $name: ident(name: $display_name:literal $(, $($t:tt)*)?)
    ) => {
        $display_name
    };
    (
        @display_name $name: ident($field_name:ident $(: $value:literal)?, $($t:tt),*)
    ) => {
        $crate::item_helper!(@display_name $name($($t),*))
    };

    // Raw
    (
        @extractable $name: ident()
    ) => {
        false
    };
    (
        @extractable $name: ident(extractable $(, $($t:tt)*)?)
    ) => {
        true
    };
    (
        @extractable $name: ident($field_name:ident $(: $value:literal)? $(, $($t:tt)*)?)
    ) => {
        $crate::item_helper!(@extractable $name($($($t)*)?))
    };

    // Fluid
    (
        @fluid $name: ident()
    ) => {
        false
    };
    (
        @fluid $name: ident(fluid $(, $($t:tt)*)?)
    ) => {
        true
    };
    (
        @fluid $name: ident($field_name:ident $(: $value:literal)? $(, $($t:tt)*)?)
    ) => {
        $crate::item_helper!(@fluid $name($($($t)*)?))
    };

    // Sink Points
    (
        @sink_points $name: ident(sink_points: $value:literal $(, $($t:tt)*)?)
    ) => {
        Some($value)
    };
    (
        @sink_points $name: ident($field_name:ident $(: $value:literal)? $(, $($t:tt)*)?)
    ) => {
        $crate::item_helper!(@sink_points $name($($($t)*)?))
    };
    (
        @sink_points $name: ident()
    ) => {
        None
    };
}

#[macro_export]
macro_rules! machine_definition {
    (
        Machine {
            $(
                $name: ident($($machine_args:tt)+)
            ),+
        }
    ) => {

        $crate::machine_helper!(@enum {
            $(
                $name($($machine_args)+)
            ),+
        });

        impl Machine {
            pub fn display_name(&self) -> &str {
                match self {
                    $(
                        Machine::$name => $crate::machine_helper!(@display_name $name($($machine_args)+))
                    ),+
                }
            }

            pub fn min_power(&self) -> u32 {
                match self {
                    $(
                        Machine::$name => $crate::machine_helper!(@min_power $name($($machine_args)+))
                    ),+
                }
            }

            pub fn max_power(&self) -> u32 {
                match self {
                    $(
                        Machine::$name => $crate::machine_helper!(@max_power $name($($machine_args)+))
                    ),+
                }
            }

            pub fn input_ports(&self) -> MachineIO {
                match self {
                    $(
                        Machine::$name => $crate::machine_helper!(@inputs $name($($machine_args)+))
                    ),+
                }
            }

            pub fn output_ports(&self) -> MachineIO {
                match self {
                    $(
                        Machine::$name => $crate::machine_helper!(@outputs $name($($machine_args)+))
                    ),+
                }
            }
        }

        impl fmt::Display for Machine {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.display_name())
            }
        }
    };
}
#[macro_export]
macro_rules! machine_helper {
    (
        @enum {
            $(
                $name: ident(name: $display_name:literal $($machine_args:tt)*)
            ),+
        }
    ) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        pub enum Machine {
            $(
                #[serde(rename = $display_name)]
                $name
            ),+
        }
    };
    // Display Name
    (
        @display_name $name: ident(name: $display_name:literal $(, $($t:tt)*)?)
    ) => {
        $display_name
    };

    (
        @display_name $name: ident($field_name:ident: $value:literal, $($t:tt),*)
    ) => {
        $crate::machine_helper!(@display_name $name($($t),*))
    };
    (
        @display_name $name: ident($field_name:ident: [$($value:tt),*], $($t:tt),*)
    ) => {
        $crate::machine_helper!(@display_name $name($($t)*))
    };

    // Min Power
    (
        @min_power $name: ident(min_power: $min_power:literal $(, $($t:tt)*)?)
    ) => {
        $min_power
    };
    (
        @min_power $name: ident(power: $power:literal $(, $($t:tt)*)?)
    ) => {
        $power
    };
    (
        @min_power $name: ident($field_name:ident: $value:literal, $($t:tt)*)
    ) => {
        $crate::machine_helper!(@min_power $name($($t)*))
    };
    (
        @min_power $name: ident($field_name:ident: [$($value:tt),*], $($t:tt)*)
    ) => {
        $crate::machine_helper!(@min_power $name($($t)*))
    };

    // Max Power
    (
        @max_power $name: ident(max_power: $max_power:literal $(, $($t:tt)*)?)
    ) => {
        $max_power
    };
    (
        @max_power $name: ident(power: $power:literal $(, $($t:tt)*)?)
    ) => {
        $power
    };
    (
        @max_power $name: ident($field_name:ident: $value:literal, $($t:tt)*)
    ) => {
        $crate::machine_helper!(@max_power $name($($t)*))
    };
    (
        @max_power $name: ident($field_name:ident: [$($value:tt),*], $($t:tt)*)
    ) => {
        $crate::machine_helper!(@max_power $name($($t)*))
    };

    // Inputs
    (
        @inputs $name: ident(inputs: [$($inputs:tt),*] $(, $($t:tt)*)?)
    ) => {
        MachineIO::new(
            $crate::machine_helper!(@sum_items [$($inputs),*]),
            $crate::machine_helper!(@sum_fluids [$($inputs),*])
        )
    };
    (
        @inputs $name: ident($field_name:ident: $value:literal, $($t:tt)*)
    ) => {
        $crate::machine_helper!(@inputs $name($($t)*))
    };
    (
        @inputs $name: ident($field_name:ident: [$($value:tt),*], $($t:tt)*)
    ) => {
        $crate::machine_helper!(@inputs $name($($t)*))
    };

    // Outputs
    (
        @outputs $name: ident(outputs: [$($outputs:tt),+] $(, $($t:tt)*)?)
    ) => {
        MachineIO::new(
            $crate::machine_helper!(@sum_items [$($outputs),+]),
            $crate::machine_helper!(@sum_fluids [$($outputs),+])
        )
    };
    (
        @outputs $name: ident($field_name:ident: $value:literal, $($t:tt)*)
    ) => {
        $crate::machine_helper!(@outputs $name($($t)*))
    };
    (
        @outputs $name: ident($field_name:ident: [$($value:tt),*], $($t:tt)*)
    ) => {
        $crate::machine_helper!(@outputs $name($($t)*))
    };

    (@sum_items []) => { 0 };
    (@sum_items [Item]) => { 1 };
    (@sum_items [Fluid]) => { 0 };
    (@sum_items [Item, $($t:tt),*]) => {
        (1 + $crate::machine_helper!(@sum_items [$($t),*]))
    };
    (@sum_items [Fluid, $($t:tt),*]) => {
        $crate::machine_helper!(@sum_items [$($t),*])
    };

    (@sum_fluids []) => { 0 };
    (@sum_fluids [Item]) => { 0 };
    (@sum_fluids [Fluid]) => { 1 };
    (@sum_fluids [Item, $($t:tt),*]) => {
        $crate::machine_helper!(@sum_fluids [$($t),*])
    };
    (@sum_fluids [Fluid, $($t:tt),*]) => {
        (1 + $crate::machine_helper!(@sum_fluids [$($t),*]))
    };
}
