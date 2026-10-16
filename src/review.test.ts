import {expect,test} from 'vitest';
import {summarize} from './review';
test('a move counts both verified transfer and source removal',()=>{expect(summarize([{path:'f',action:'MoveToB',reason:'',a:null,b:null}])).toEqual({copies:1,deletes:1,conflicts:0,unchanged:0})});
