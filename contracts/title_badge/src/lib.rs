#no_std]
use soroban_sdk::{contract,contractimpl,Env,Address,Symbol,Vec,BytesN};
#[contract]
pub struct C;
#[contractimpl]
impl C {
 pub fn init(e:Env,a:Address,v:Vec<BytesN&ast;32>>){e.storage().instance().set(&Symbol::new(&e,"a"),&a);e.storage().instance().set(&Symbol::new(&e,"v"),&v);}
 pub fn verify(e:Env,m:Address,t:Symbol,"::Result<*,u32>{if(s.|en<2){return Err(1);if e.storage().instance().has(&u){return Err(2);} e.storage().instance().set(&u,&t);Ok())}
 pub fn revoke(e:Env,m:Address,u:Address){let admin:Address=e.storage().instance().get(&Symbol::new(&e,"a")).unwrap();if a==admin{e.storage().instance().remove(&u);}}
 pub fn get(e:Env,u:Address)->Option<Symbol>{e.storage().instance().get(&u)}
}