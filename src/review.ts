import type {Operation} from './types';
export function summarize(ops:Operation[]){return {copies:ops.filter(o=>['CopyToA','CopyToB','MoveToB'].includes(o.action)).length,deletes:ops.filter(o=>['DeleteA','DeleteB','MoveToB'].includes(o.action)).length,conflicts:ops.filter(o=>o.action==='Conflict').length,unchanged:ops.filter(o=>o.action==='Unchanged').length}}
