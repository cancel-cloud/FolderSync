use crate::*;
use anyhow::Result;
use rusqlite::{Connection,params,OptionalExtension};
use std::{path::{Path,PathBuf},collections::BTreeSet};
pub struct Index { pub db:Connection, pub storage:PathBuf }
impl Index {
 pub fn open(storage:&Path)->Result<Self>{std::fs::create_dir_all(storage.join("versions"))?;let db=Connection::open(storage.join("foldersync.sqlite3"))?;
 db.busy_timeout(std::time::Duration::from_secs(5))?;
 db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
 CREATE TABLE IF NOT EXISTS tasks(id TEXT PRIMARY KEY,json TEXT NOT NULL,root_a TEXT NOT NULL,root_b TEXT NOT NULL,generation INTEGER NOT NULL DEFAULT 0);
 CREATE TABLE IF NOT EXISTS entries(task TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,side TEXT NOT NULL,path TEXT NOT NULL,parent TEXT NOT NULL,json TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(task,side,path));
 CREATE INDEX IF NOT EXISTS entries_parent ON entries(task,side,parent);
 CREATE TABLE IF NOT EXISTS baseline(task TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,path TEXT NOT NULL,digest TEXT,PRIMARY KEY(task,path));
 CREATE TABLE IF NOT EXISTS merkle(task TEXT NOT NULL,side TEXT NOT NULL,path TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(task,side,path));
 CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY,task TEXT NOT NULL,json TEXT NOT NULL,status TEXT NOT NULL,created INTEGER NOT NULL);
 CREATE INDEX IF NOT EXISTS runs_task ON runs(task,created DESC);
 CREATE TABLE IF NOT EXISTS journal(run TEXT NOT NULL,path TEXT NOT NULL,json TEXT NOT NULL,status TEXT NOT NULL,error TEXT,PRIMARY KEY(run,path));
 CREATE TABLE IF NOT EXISTS versions(id TEXT PRIMARY KEY,task TEXT NOT NULL,json TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS dirty(task TEXT NOT NULL,side TEXT NOT NULL,path TEXT NOT NULL,PRIMARY KEY(task,side,path));
 CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,json TEXT NOT NULL);
 PRAGMA user_version=1;
 UPDATE runs SET status='Interrupted' WHERE status='Applying';")?;Ok(Self{db,storage:storage.to_path_buf()})}
 pub fn tasks(&self)->Result<Vec<Task>>{let mut s=self.db.prepare("SELECT json FROM tasks ORDER BY id")?;let rows=s.query_map([],|r|r.get::<_,String>(0))?;rows.map(|r|Ok(serde_json::from_str(&r?)?)).collect()}
 pub fn task(&self,id:&str)->Result<Task>{Ok(serde_json::from_str(&self.db.query_row("SELECT json FROM tasks WHERE id=?",[id],|r|r.get::<_,String>(0))?)?)}
 pub fn generation(&self,id:&str)->Result<i64>{Ok(self.db.query_row("SELECT generation FROM tasks WHERE id=?",[id],|r|r.get(0))?)}
 pub fn roots(&self,id:&str)->Result<(String,String)>{Ok(self.db.query_row("SELECT root_a,root_b FROM tasks WHERE id=?",[id],|r|Ok((r.get(0)?,r.get(1)?)))?)}
 pub fn tree(&self,task:&str,side:&str)->Result<Tree>{let mut s=self.db.prepare("SELECT path,json FROM entries WHERE task=? AND side=?")?;let rows=s.query_map(params![task,side],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;rows.map(|r|{let(k,j)=r?;Ok((k,serde_json::from_str(&j)?))}).collect()}
 pub fn put_entry(&self,task:&str,side:&str,key:&str,entry:Option<&Entry>)->Result<()>{if let Some(e)=entry{let parent=key.rsplit_once('/').map(|p|p.0).unwrap_or("");self.db.execute("INSERT OR REPLACE INTO entries VALUES(?,?,?,?,?,?)",params![task,side,key,parent,serde_json::to_string(e)?,e.digest])?;}else{self.db.execute("DELETE FROM entries WHERE task=? AND side=? AND path=?",params![task,side,key])?;}Ok(())}
 pub fn put_tree(&mut self,task:&str,side:&str,tree:&Tree)->Result<()>{let tx=self.db.transaction()?;tx.execute("DELETE FROM entries WHERE task=? AND side=?",params![task,side])?;{let mut s=tx.prepare("INSERT INTO entries VALUES(?,?,?,?,?,?)")?;for(k,e)in tree{s.execute(params![task,side,k,k.rsplit_once('/').map(|p|p.0).unwrap_or(""),serde_json::to_string(e)?,e.digest])?;}}tx.commit()?;let keys=tree.keys().cloned().collect();self.update_ancestors(task,side,&keys)}
 pub fn update_ancestors(&self,task:&str,side:&str,keys:&BTreeSet<String>)->Result<()>{let mut parents=BTreeSet::from([String::new()]);for key in keys{let mut p=key.as_str();while let Some((parent,_))=p.rsplit_once('/'){parents.insert(parent.to_string());p=parent;}}
 let mut parents:Vec<_>=parents.into_iter().collect();parents.sort_by_key(|p|std::cmp::Reverse(p.matches('/').count()+usize::from(!p.is_empty())));
 for parent in parents{let mut h=blake3::Hasher::new();let mut s=self.db.prepare("SELECT path,digest,json FROM entries WHERE task=? AND side=? AND parent=? ORDER BY path")?;let rows=s.query_map(params![task,side,parent],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?;
 for row in rows{let(path,mut digest,json)=row?;let e:Entry=serde_json::from_str(&json)?;if e.directory{digest=self.db.query_row("SELECT digest FROM merkle WHERE task=? AND side=? AND path=?",params![task,side,path],|r|r.get(0)).optional()?.unwrap_or(digest);}h.update(path.as_bytes());h.update(&[0]);h.update(digest.as_bytes());h.update(&[0]);}
 self.db.execute("INSERT OR REPLACE INTO merkle VALUES(?,?,?,?)",params![task,side,parent,h.finalize().to_hex().to_string()])?;}Ok(())}
 pub fn baseline(&self,task:&str)->Result<std::collections::BTreeMap<String,Option<String>>>{let mut s=self.db.prepare("SELECT path,digest FROM baseline WHERE task=?")?;let rows=s.query_map([task],|r|Ok((r.get(0)?,r.get(1)?)))?;Ok(rows.collect::<rusqlite::Result<_>>()?)}
 pub fn plan(&self,id:&str)->Result<Plan>{Ok(serde_json::from_str(&self.db.query_row("SELECT json FROM runs WHERE id=?",[id],|r|r.get::<_,String>(0))?)?)}
 pub fn put_plan(&self,p:&Plan)->Result<()>{self.db.execute("INSERT INTO runs VALUES(?,?,?,?,?)",params![p.id,p.task.id,serde_json::to_string(p)?,p.status,p.created_at as i64])?;Ok(())}
 pub fn history(&self,task:&str)->Result<Vec<Plan>>{let mut s=self.db.prepare("SELECT json,status FROM runs WHERE task=? ORDER BY created DESC,rowid DESC LIMIT 100")?;let rows=s.query_map([task],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;rows.map(|r|{let(j,status)=r?;let mut p:Plan=serde_json::from_str(&j)?;p.status=status;Ok(p)}).collect()}
 pub fn versions(&self,task:&str)->Result<Vec<Version>>{let mut s=self.db.prepare("SELECT json FROM versions WHERE task=? ORDER BY rowid DESC")?;let rows=s.query_map([task],|r|r.get::<_,String>(0))?;rows.map(|r|Ok(serde_json::from_str(&r?)?)).collect()}
}

