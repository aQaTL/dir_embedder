extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse_macro_input;
use syn::{
	parse::{Parse, ParseStream, Result},
	token::Comma,
	Ident, LitStr,
};

struct InputDir {
	name: Ident,
	path: String,
}

impl Parse for InputDir {
	fn parse(input: ParseStream) -> Result<Self> {
		let name = input.parse::<Ident>()?;
		let _ = input.parse::<Comma>()?;
		let path = input.parse::<LitStr>()?.value();

		Ok(InputDir { name, path })
	}
}

#[proc_macro]
pub fn embed_dir(input: TokenStream) -> TokenStream {
	let input_dir: InputDir = parse_macro_input!(input as InputDir);
	let name = input_dir.name;
	let dir_path = input_dir.path;

	let lits = dir_iter::DirIter::new(&dir_path)
		.unwrap()
		.into_iter()
		.filter_map(std::result::Result::ok)
		.filter(|(_, metadata)| metadata.is_file())
		.map(|(entry, _metadata)| {
			let path = entry.path();
			let lit = LitStr::new(
				path.strip_prefix(&dir_path).unwrap().to_str().unwrap(),
				Span::call_site(),
			);
			let lit_absolute = LitStr::new(
				path.canonicalize().unwrap().to_str().unwrap(),
				Span::call_site(),
			);
			(lit, lit_absolute)
		})
		.map(|(lit, lit_absolute)| {
			quote! {
				hm.insert(#lit, include_bytes!(#lit_absolute).as_slice());
			}
		})
		.collect::<Vec<_>>();

	let output = quote! {
		lazy_static::lazy_static! {
			pub static ref #name: ::std::collections::HashMap<&'static str, &'static [u8]> = {
				let mut hm = ::std::collections::HashMap::new();
				#(#lits)*
				hm
			};
		}
	};

	proc_macro::TokenStream::from(output)
}

mod dir_iter {
	use std::{
		fs::{DirEntry, Metadata},
		io,
		path::Path,
	};

	pub struct DirIter {
		stack: Vec<DirEntry>,
	}

	impl DirIter {
		pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
			let mut dir_iter = DirIter { stack: Vec::new() };
			dir_iter.read_dir(path.as_ref())?;
			Ok(dir_iter)
		}

		fn read_dir(&mut self, dir: &Path) -> io::Result<()> {
			let mut entries = dir.read_dir()?.filter_map(Result::ok).collect::<Vec<_>>();
			self.stack.append(&mut entries);
			Ok(())
		}
	}

	impl Iterator for DirIter {
		type Item = io::Result<(DirEntry, Metadata)>;

		fn next(&mut self) -> Option<Self::Item> {
			let last = match self.stack.pop() {
				Some(last) => last,
				None => return None,
			};

			let metadata = match last.metadata() {
				Ok(m) => m,
				Err(e) => return Some(Err(e)),
			};

			if metadata.is_dir() {
				if let Err(e) = self.read_dir(&last.path()) {
					return Some(Err(e));
				}
			}

			Some(Ok((last, metadata)))
		}
	}
}
