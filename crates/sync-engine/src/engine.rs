use crate::{*,fs::{self as syncfs,read_entry,scan,validate_roots,path_key},index::Index};
use anyhow::{Result,ensure,bail,Context};
use rusqlite::{params,OptionalExtension};
use std::{collections::BTreeSet,path::Path,fs,sync::{Arc,atomic::{AtomicBool,Ordering}}};
pub struct Engine { pub(crate) index:Index, pub cancel:Arc<AtomicBool> }
impl Engine {
 pub fn open(storage:&Path)->Result<Self>{Ok(Self{index:Index::open(storage)?,cancel:Arc::new(AtomicBool::new(false))})}
 pub fn storage(&self)->&Path{&self.index.storage}
 pub fn save_task(&mut self,t:&Task)->Result<()>{
 ensure!(!t.name.trim().is_empty(),"Name is required");ensure!(uuid::Uuid::parse_str(&t.id).is_ok(),"Invalid task ID");ensure!(t.debounce_secs<=3600 && t.retries<=10 && t.history_cap>0 && t.history_cap<=1000,"Invalid advanced settings");syncfs::Matcher::new(&t.scope)?;let (a,b)=validate_roots(t,self.storage())?;
 if let Ok(old)=self.index.task(&t.id){ensure!(old.a==t.a && old.b==t.b && old.mode==t.mode,"Changing roots or mode requires a new task to preserve baseline semantics");}
 for other in self.tasks()?.into_iter().filter(|o|o.id!=t.id){if t.monitor || other.monitor || !t.schedule.kind.is_empty() || !other.schedule.kind.is_empty(){for x in [&t.a,&t.b]{for y in [&other.a,&other.b]{ensure!(!syncfs::overlap(&fs::canonicalize(x)?,&fs::canonicalize(y)?),"Automatic tasks cannot use overlapping roots");}}}}
 self.index.db.execute("INSERT INTO tasks(id,json,root_a,root_b) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET json=excluded.json,generation=generation+1",params![t.id,serde_json::to_string(t)?,a,b])?;Ok(())}
 pub fn tasks(&self)->Result<Vec<Task>>{self.index.tasks()}
 pub(crate) fn validate(&self,t:&Task)->Result<(String,String)>{let roots=validate_roots(t,self.storage())?;ensure!(roots==self.index.roots(&t.id)?,"Root identity changed; review and recreate this task");Ok(roots)}
 pub fn preview(&mut self,id:&str)->Result<Plan>{let t=self.index.task(id)?;self.validate(&t)?;let mut m=Metrics{reconciliations:1,..Metrics::default()};let a=scan(&t.a,&t.scope,&mut m)?;let b=scan(&t.b,&t.scope,&mut m)?;self.index.put_tree(id,"A",&a)?;self.index.put_tree(id,"B",&b)?;let keys=a.keys().chain(b.keys()).chain(self.index.baseline(id)?.keys()).cloned().collect();self.make_plan(t,&a,&b,keys,m)}
 pub(crate) fn make_plan(&self,t:Task,a:&Tree,b:&Tree,keys:BTreeSet<String>,metrics:Metrics)->Result<Plan>{let baseline=self.index.baseline(&t.id)?;let roots=self.validate(&t)?;let mut operations=Vec::new();let matcher=syncfs::Matcher::new(&t.scope)?;
 for key in keys{let ae=a.get(&key);let be=b.get(&key);let path=ae.or(be).map(|e|e.path.clone()).unwrap_or(key.clone());if !matcher.allows(&path,ae.or(be).is_some_and(|e|e.directory)){continue}
 let mut action=classify(t.mode,ae.map(|e|e.digest.as_str()),be.map(|e|e.digest.as_str()),baseline.get(&key).and_then(|x|x.as_deref()));
 if ae.zip(be).is_some_and(|(x,y)|x.directory!=y.directory){action=Action::Conflict;}
 operations.push(Operation{path,action,reason:match action{Action::Conflict=>"Both sides differ from a common baseline, or there is no safe baseline",Action::DeleteA|Action::DeleteB=>"Reviewed deletion; a protected version will be kept",Action::MoveToB=>"Verify destination before removing source",Action::Unchanged=>"No transfer required",_=>"One-sided creation or change"}.into(),a:ae.cloned(),b:be.cloned()});}
 let p=Plan{id:uuid::Uuid::new_v4().to_string(),baseline_generation:self.index.generation(&t.id)?,task:t,root_a:roots.0,root_b:roots.1,operations,metrics,created_at:now(),status:"Preview".into()};self.index.put_plan(&p)?;Ok(p)}
 pub fn resolve(&mut self,id:&str,path:&str,choice:&str)->Result<Plan>{let mut p=self.index.plan(id)?;ensure!(p.status=="Preview","Only previews can be revised");let o=p.operations.iter_mut().find(|o|o.path==path && o.action==Action::Conflict).context("Conflict not found")?;o.action=match choice{"a" if o.a.is_some()=>Action::CopyToB,"b" if o.b.is_some()=>Action::CopyToA,"ignore"=>Action::Conflict,_=>bail!("Choose an existing side or leave unresolved")};o.reason="Explicit conflict choice; Apply is still required".into();p.id=uuid::Uuid::new_v4().to_string();self.index.put_plan(&p)?;Ok(p)}
 pub fn apply(&mut self,id:&str,destructive:bool)->Result<Plan>{
 self.cancel.store(false,Ordering::Relaxed);let mut p=self.index.plan(id)?;ensure!(p.status=="Preview","Plan already applied or interrupted; create a new preview");ensure!(self.index.task(&p.task.id)?==p.task && self.index.generation(&p.task.id)?==p.baseline_generation,"Task or baseline changed; preview again");let roots=self.validate(&p.task)?;ensure!(roots==(p.root_a.clone(),p.root_b.clone()),"Roots changed");ensure!(destructive || !p.operations.iter().any(|o|matches!(o.action,Action::DeleteA|Action::DeleteB|Action::MoveToB) || o.reason.starts_with("Explicit")),"Destructive review is required");
 for o in &p.operations{for(root,expected)in [(&p.task.a,&o.a),(&p.task.b,&o.b)]{let actual=read_entry(root,&o.path,&p.task.scope,&mut p.metrics)?;ensure!(&actual==expected,"Stale input: {}; preview again",o.path);}}
 self.index.db.execute("UPDATE runs SET status='Applying' WHERE id=?",[id])?;
 p.operations.sort_by_key(|o|{let deleting=matches!(o.action,Action::DeleteA|Action::DeleteB);let dir=o.a.as_ref().or(o.b.as_ref()).is_some_and(|e|e.directory);(if deleting{2}else if dir{0}else{1},if deleting{usize::MAX-o.path.matches('/').count()}else{o.path.matches('/').count()})});
 let result=(||->Result<()>{
 for o in &p.operations{if o.action==Action::Conflict{continue}ensure!(!self.cancel.load(Ordering::Relaxed),"Cancelled at a safe operation boundary");self.validate(&p.task)?;self.index.db.execute("INSERT OR REPLACE INTO journal(run,path,json,status) VALUES(?,?,?,'Pending')",params![id,o.path,serde_json::to_string(o)?])?;
 match o.action{
 Action::CopyToB|Action::MoveToB=>self.copy(&p.task,&o.path,"A","B",o.a.as_ref().context("Source missing")?,o.b.as_ref(),&mut p.metrics)?,
 Action::CopyToA=>self.copy(&p.task,&o.path,"B","A",o.b.as_ref().context("Source missing")?,o.a.as_ref(),&mut p.metrics)?,
 Action::DeleteA=>self.remove(&p.task,&o.path,"A",o.a.as_ref().context("Missing delete input")?,&mut p.metrics)?,
 Action::DeleteB=>self.remove(&p.task,&o.path,"B",o.b.as_ref().context("Missing delete input")?,&mut p.metrics)?,_=>{}}
 if o.action==Action::MoveToB && !o.a.as_ref().is_some_and(|e|e.directory){self.remove(&p.task,&o.path,"A",o.a.as_ref().unwrap(),&mut p.metrics)?;}
 self.index.db.execute("UPDATE journal SET status='Complete' WHERE run=? AND path=?",params![id,o.path])?;
 let (a,b)=(read_entry(&p.task.a,&o.path,&p.task.scope,&mut p.metrics)?,read_entry(&p.task.b,&o.path,&p.task.scope,&mut p.metrics)?);
 let digest=if p.task.mode==Mode::TwoWay{if a.as_ref().map(|e|&e.digest)!=b.as_ref().map(|e|&e.digest){continue}a.as_ref().map(|e|e.digest.clone())}else if p.task.mode==Mode::Move{None}else{a.as_ref().filter(|x|b.as_ref().is_some_and(|y|x.digest==y.digest)).map(|e|e.digest.clone())};
 let tx=self.index.db.transaction()?;tx.execute("INSERT OR REPLACE INTO baseline VALUES(?,?,?)",params![p.task.id,path_key(&o.path),digest])?;tx.commit()?;self.index.put_entry(&p.task.id,"A",&path_key(&o.path),a.as_ref())?;self.index.put_entry(&p.task.id,"B",&path_key(&o.path),b.as_ref())?;
 }
 let mut dirs:Vec<_>=p.operations.iter().filter(|o|o.action==Action::MoveToB && o.a.as_ref().is_some_and(|e|e.directory)).collect();dirs.sort_by_key(|o|std::cmp::Reverse(o.path.matches('/').count()));for o in dirs{let path=syncfs::safe_path(&p.task.a,&o.path)?;fs::remove_dir(path)?;self.index.put_entry(&p.task.id,"A",&path_key(&o.path),None)?;}
 Ok(())})();
 p.status=match &result{Ok(_)=>if p.operations.iter().any(|o|o.action==Action::Conflict){"Attention".into()}else{"Applied".into()},Err(e)=>format!("Failed: {e:#}")};
 self.index.db.execute("UPDATE tasks SET generation=generation+1 WHERE id=?",[&p.task.id])?;self.index.db.execute("UPDATE runs SET json=?,status=? WHERE id=?",params![serde_json::to_string(&p)?,p.status,p.id])?;
 let keys=p.operations.iter().map(|o|path_key(&o.path)).collect();self.index.update_ancestors(&p.task.id,"A",&keys)?;self.index.update_ancestors(&p.task.id,"B",&keys)?;result?;Ok(p)}
 fn backup(&self,t:&Task,path:&str,side:&str,entry:&Entry,m:&mut Metrics)->Result<()> {if entry.directory{return Ok(())}let root=if side=="A"{&t.a}else{&t.b};let source=syncfs::safe_path(root,path)?;let id=uuid::Uuid::new_v4().to_string();let stored=self.storage().join("versions").join(&id);self.transfer(&source,&stored,&entry.digest,t,m)?;let v=Version{id,task_id:t.id.clone(),side:side.into(),path:path.into(),digest:entry.digest.clone(),stored,created_at:now()};self.index.db.execute("INSERT INTO versions VALUES(?,?,?)",params![v.id,t.id,serde_json::to_string(&v)?])?;Ok(())}
 fn copy(&self,t:&Task,path:&str,from:&str,to:&str,expected:&Entry,dest:Option<&Entry>,m:&mut Metrics)->Result<()> {let source_root=if from=="A"{&t.a}else{&t.b};let target_root=if to=="A"{&t.a}else{&t.b};ensure!(read_entry(source_root,path,&t.scope,m)?.as_ref()==Some(expected),"Source changed before transfer");ensure!(read_entry(target_root,path,&t.scope,m)?.as_ref()==dest,"Destination changed before transfer");let source=syncfs::safe_path(source_root,path)?;let target=syncfs::safe_path(target_root,path)?;if expected.directory{fs::create_dir_all(target)?;return Ok(())}if let Some(old)=dest{if old.digest==expected.digest{return Ok(())}self.backup(t,path,to,old,m)?;}fs::create_dir_all(target.parent().context("Missing parent")?)?;self.transfer(&source,&target,&expected.digest,t,m)?;Ok(())}
 fn remove(&self,t:&Task,path:&str,side:&str,expected:&Entry,m:&mut Metrics)->Result<()> {let root=if side=="A"{&t.a}else{&t.b};let actual=read_entry(root,path,&t.scope,m)?.context("Delete input vanished")?;ensure!(actual.digest==expected.digest && actual.directory==expected.directory,"Delete input changed");let target=syncfs::safe_path(root,path)?;if expected.directory{fs::remove_dir(target)?;}else{self.backup(t,path,side,expected,m)?;ensure!(read_entry(root,path,&t.scope,m)?.as_ref()==Some(expected),"Delete input changed during backup");fs::remove_file(target)?;}Ok(())}
 pub fn history(&self,task:&str)->Result<Vec<Plan>>{self.index.history(task)}
 pub fn versions(&self,task:&str)->Result<Vec<Version>>{self.index.versions(task)}
 pub fn delete_task(&mut self,id:&str)->Result<()>{self.index.db.execute("DELETE FROM tasks WHERE id=?",[id])?;Ok(())}
 pub fn export(&self)->Result<String>{Ok(serde_json::to_string_pretty(&serde_json::json!({"schema":1,"tasks":self.tasks()?}))?)}
 pub fn import(&mut self,json:&str)->Result<usize>{let value:serde_json::Value=serde_json::from_str(json)?;ensure!(value["schema"]==1,"Unsupported schema");let mut tasks:Vec<Task>=serde_json::from_value(value["tasks"].clone())?;ensure!(tasks.len()<=1000,"Too many tasks");for t in &mut tasks{t.id=uuid::Uuid::new_v4().to_string();t.monitor=false;t.login=false;t.schedule=Schedule::default();validate_roots(t,self.storage())?;syncfs::Matcher::new(&t.scope)?;}for t in &tasks{self.save_task(t)?;}Ok(tasks.len())}
 pub fn setting(&self,key:&str)->Result<Option<String>>{Ok(self.index.db.query_row("SELECT json FROM settings WHERE key=?",[key],|r|r.get(0)).optional()?)}
 pub fn set_setting(&self,key:&str,json:&str)->Result<()>{self.index.db.execute("INSERT OR REPLACE INTO settings VALUES(?,?)",params![key,json])?;Ok(())}
}
