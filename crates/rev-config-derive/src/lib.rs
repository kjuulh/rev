use heck::{ToShoutySnakeCase, ToSnakeCase};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(AppConfig)]
pub fn app_config_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as TokenStream2);

    let ast = syn::parse2::<DeriveInput>(input.clone()).unwrap();

    let struct_name = &ast.ident;
    let namespace = syn::Ident::new(
        &format!("{}", &struct_name).to_snake_case(),
        struct_name.span(),
    );

    let (try_gen, field_gen, cli_gen, cli_args_gen, env_gen, config_gen) = match &ast.data {
        syn::Data::Struct(data_struct) => data_struct
            .fields
            .iter()
            .map(|f| {
                let field_name = f.ident.as_ref().unwrap();
                let field_type = &f.ty;

                let try_gen = quote! {
                    #field_name: value.#field_name.ok_or(::anyhow::anyhow!(format!("expected {} to be set", stringify!(#field_name))))?,
                };

                let field_gen = quote! {
                    pub #field_name: Option<#field_type>,
                };

                let cli_gen = quote! {
                    if let Some(#field_name) = matches.remove_one::<#field_type>(stringify!(#field_name)) {
                        s.#field_name = Some(#field_name);
                    }
                };

                let cli_args_gen = quote! {
                    .arg(
                        ::clap::Arg::new(stringify!(#field_name))
                            .long(stringify!(#field_name))
                            .action(::clap::ArgAction::Set)
                            .help_heading("Config")
                            .global(true)
                    )
                };

                let screaming_field_name = syn::Ident::new(&field_name.to_string().to_shouty_snake_case(), field_name.span());
                let env_gen = quote! {
                    if let Ok(item) =  std::env::var(format!("REV_{}", stringify!(#screaming_field_name))) {
                        self.#field_name = Some(item);
                    }
                };

                let config_gen = quote! {
                    if let Some(item) = config.get(stringify!(#field_name)).and_then(|i| i.entries().first()).map(|i| i.value()) {
                        tracing::debug!("found item: {}", item);
                        self.#field_name = item.as_string().map(|i| i.to_string());
                    }
                };

                (try_gen, field_gen, cli_gen, cli_args_gen, env_gen, config_gen)
            })
        .fold(
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            |(mut try_gen_acc, mut field_gen_acc, mut left_acc, mut right_acc, mut up_acc, mut down_acc),
              (try_gen, field_gen, left, right, up, down)| {
                    try_gen_acc.push(try_gen);
                field_gen_acc.push(field_gen);
                left_acc.push(left);
                right_acc.push(right);
                up_acc.push(up);
                down_acc.push(down);
        (try_gen_acc, field_gen_acc, left_acc, right_acc, up_acc, down_acc)
    }),
        _ => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
    };

    let try_gen = try_gen.into_iter().collect::<TokenStream2>();
    let field_gen = field_gen.into_iter().collect::<TokenStream2>();
    let cli_gen = cli_gen.into_iter().collect::<TokenStream2>();
    let cli_args_gen = cli_args_gen.into_iter().collect::<TokenStream2>();
    let env_gen = env_gen.into_iter().collect::<TokenStream2>();
    let config_gen = config_gen.into_iter().collect::<TokenStream2>();

    let expanded = quote! {
        impl #struct_name {
            pub fn from(conf: #namespace::#struct_name) -> ::anyhow::Result<Self> {
                use ::anyhow::Context;

                let c = conf.try_into().context("failed to read variables from env")?;

                Ok(c)
            }

            pub fn get_config_file_path(&self) -> ::std::path::PathBuf {
                std::env::var("REV_CONFIG_HOME")
                    .map(::std::path::PathBuf::from)
                    .unwrap_or_else(|_| {
                        let project = ::directories::ProjectDirs::from("io", "kjuulh", "rev")
                            .expect("to be able to find XDG home variables");

                        project.config_dir().to_path_buf()
                   })
            }

        }

        impl TryFrom<#namespace::#struct_name> for #struct_name {
            type Error = ::anyhow::Error;

            fn try_from(value: inner_application_config::#struct_name) -> Result<Self, Self::Error> {
                Ok(Self {
                    #try_gen
                })
            }
        }

        pub mod #namespace {
            #[derive(Default)]
            pub struct #struct_name {
                config_file: ::std::path::PathBuf,

                #field_gen
            }

            impl ::clap::FromArgMatches for #struct_name {
                fn from_arg_matches(matches: &::clap::ArgMatches) -> Result<Self, ::clap::error::Error> {
                    let mut matches = matches.clone();
                    Self::from_arg_matches_mut(&mut matches)
                }
                fn from_arg_matches_mut(matches: &mut ::clap::ArgMatches) -> Result<Self, ::clap::error::Error> {
                    use ::rev_config::{Env, ConfigFile};

                    let mut s = Self::default();

                    let config = if let Some(config_file) = matches.remove_one::<String>("config-file") {
                        ::std::path::PathBuf::from(config_file)
                    } else {
                        let project_home = std::env::var("REV_CONFIG_HOME")
                            .map(::std::path::PathBuf::from)
                            .unwrap_or_else(|_| {
                                let project = ::directories::ProjectDirs::from("io", "kjuulh", "rev")
                                    .expect("to be able to find XDG home variables");

                                project.config_dir().to_path_buf()
                            });


                        project_home.join("rev.kdl")
                    };

                    if let Err(e) = s.set_from_config_file(&config) {
                        ::tracing::warn!("failed to read config from file: {e}");
                    }
                    if let Err(e) = s.set_from_env() {
                        ::tracing::warn!("failed to read config from env: {e}");
                    }

                    #cli_gen

                    Ok(s)
                }
                fn update_from_arg_matches(&mut self, matches: &::clap::ArgMatches) -> Result<(), ::clap::error::Error> {
                    let mut matches = matches.clone();
                    self.update_from_arg_matches_mut(&mut matches)
                }
                fn update_from_arg_matches_mut(&mut self, matches: &mut ::clap::ArgMatches) -> Result<(), ::clap::error::Error> {
                    use ::rev_config::{Env, ConfigFile};

                    let mut s = Self::default();

                    let config = if let Some(config_file) = matches.remove_one::<String>("config-file") {
                        ::std::path::PathBuf::from(config_file)
                    } else {
                        let project_home = std::env::var("REV_CONFIG_HOME")
                            .map(::std::path::PathBuf::from)
                            .unwrap_or_else(|_| {
                                let project = ::directories::ProjectDirs::from("io", "kjuulh", "rev")
                                    .expect("to be able to find XDG home variables");

                                project.config_dir().to_path_buf()
                            });


                        project_home.join("rev.kdl")
                    };

                    if let Err(e) = self.set_from_config_file(&config) {
                        ::tracing::warn!("failed to read config from file: {e}");
                    }
                    if let Err(e) = self.set_from_env() {
                        ::tracing::warn!("failed to read config from env: {e}");
                    }

                    #cli_gen

                    Ok(())
                }
            }

            impl ::clap::Args for #struct_name {
                fn augment_args(cmd: ::clap::Command) -> ::clap::Command {
                    cmd
                        .arg(
                            ::clap::Arg::new("config-file")
                                .long("config-file")
                                .action(::clap::ArgAction::Set)
                                .help_heading("Config")
                                .global(true)
                        )
                    #cli_args_gen
                }
                fn augment_args_for_update(cmd: ::clap::Command) -> ::clap::Command {
                    cmd
                        .arg(
                            ::clap::Arg::new("config-file")
                                .long("config-file")
                                .action(::clap::ArgAction::Set)
                                .help_heading("Config")
                                .global(true)
                        )
                    #cli_args_gen
                }
            }

            impl ::rev_config::Env for #struct_name {
                fn set_from_env(&mut self) -> Result<(), ::rev_config::EnvError> {
                    #env_gen

                    Ok(())
                }

            }

            impl ::rev_config::ConfigFile for #struct_name {
                fn set_from_config_file(&mut self, config_file: &::std::path::Path) -> Result<(), ::rev_config::ConfigFileError> {
                    pub use anyhow::Context;

                    tracing::trace!("looking for kdl config at: {}", config_file.display());
                    let file_content = ::std::fs::read_to_string(config_file)
                        .context("failed to read confile file")
                        .map_err(|e| ::rev_config::ConfigFileError::ConfigFileError(e))?;

                    let doc: ::kdl::KdlDocument = file_content.parse()
                        .context("failed to parse kdl config file")
                        .map_err(|e| ::rev_config::ConfigFileError::ConfigFileError(e))?;

                    tracing::trace!("found doc at: {:#?}", doc);

                    if let Some(config) = doc.get("config") {
                        if let Some(config) = config.children() {
                            tracing::debug!("found config");

                            #config_gen
                        }
                    }

                    Ok(())
                }
            }
        }
    };

    TokenStream::from(expanded)
}
