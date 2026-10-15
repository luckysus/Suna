import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {createContext,useContext} from 'react';
import {expect,it,vi} from 'vitest';
import {deferredPage} from './deferredPage';

it('loads only after navigation mounts the page and passes its props and context',async()=>{
  const Context=createContext('default');
  const importer=vi.fn().mockResolvedValue({default:({label}:{label:string})=><p>{label} · {useContext(Context)}</p>});
  const Page=deferredPage<{label:string}>(importer);expect(importer).not.toHaveBeenCalled();
  render(<Context.Provider value="合成项目"><Page label="延迟模块"/></Context.Provider>);
  expect(screen.getByRole('status')).toHaveTextContent('正在加载页面');expect(await screen.findByText('延迟模块 · 合成项目')).toBeInTheDocument();expect(importer).toHaveBeenCalledTimes(1);
});
it('offers explicit document reload after a failed asset fetch without exposing raw error details',async()=>{
  const importer=vi.fn().mockRejectedValue(new Error('synthetic-private-asset-location')),reload=vi.fn();
  const Page=deferredPage(importer,reload);render(<Page/>);
  expect(await screen.findByRole('alert')).toHaveTextContent('页面加载失败');expect(screen.queryByText(/synthetic-private/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:'重新加载页面'}));expect(reload).toHaveBeenCalledTimes(1);expect(importer).toHaveBeenCalledTimes(1);
});
it('shares an in-flight import and retains a successful module across unmounts',async()=>{
  let release!:(v:{default:()=>React.JSX.Element})=>void;
  const importer=vi.fn().mockImplementation(()=>new Promise(resolve=>{release=resolve;}));
  const Page=deferredPage(importer);const first=render(<><Page/><Page/></>);await waitFor(()=>expect(importer).toHaveBeenCalledTimes(1));
  first.unmount();await act(async()=>release({default:()=> <p>复用模块</p>}));render(<Page/>);
  expect(screen.getByText('复用模块')).toBeInTheDocument();expect(importer).toHaveBeenCalledTimes(1);
});
it('does not let a late previous module replace the current page',async()=>{
  let release!:(v:{default:()=>React.JSX.Element})=>void;
  const Old=deferredPage(vi.fn().mockImplementation(()=>new Promise(resolve=>{release=resolve;})));
  const Current=deferredPage(async()=>({default:()=> <p>当前科研页面</p>}));
  const node=render(<Old/>);await waitFor(()=>expect(release).toBeDefined());node.rerender(<Current/>);await screen.findByText('当前科研页面');
  await act(async()=>release({default:()=> <p>过期页面</p>}));expect(screen.getByText('当前科研页面')).toBeInTheDocument();expect(screen.queryByText('过期页面')).not.toBeInTheDocument();
});
