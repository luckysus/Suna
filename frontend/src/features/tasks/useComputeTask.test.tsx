import { beforeEach, expect, it, vi } from "vitest";
import { desktop, type SchedulerProgressEvent, type BackgroundTask } from "../../bridge/desktop";
import { waitForComputeResult } from "./useComputeTask";
vi.mock("../../bridge/desktop", () => ({ desktop: { listenSchedulerProgress:vi.fn(), getBackgroundTask:vi.fn(), listBackgroundTasks:vi.fn() } }));
const task = (state:BackgroundTask["state"]) => ({id:"task",kind:"compute_train",state,progress:state==="completed"?100:25,error_code:state==="failed"?"bad_data":null}) as BackgroundTask;
let listener:(event:SchedulerProgressEvent)=>void;
const unlisten=vi.fn();
beforeEach(() => { vi.clearAllMocks();vi.mocked(desktop.listenSchedulerProgress).mockImplementation(async (fn) => {listener=fn;return unlisten;}); });
it("reports failure immediately with its cause and removes listeners", async () => {
  vi.mocked(desktop.getBackgroundTask).mockResolvedValue(task("failed"));
  await expect(waitForComputeResult("task",vi.fn())).rejects.toThrow("bad_data");
  expect(unlisten).toHaveBeenCalled();
});
it("wakes on progress and returns the completed result", async () => {
  vi.mocked(desktop.getBackgroundTask).mockResolvedValueOnce(task("running")).mockResolvedValue(task("completed"));
  const result={predictions:[1,2]};const read=vi.fn().mockResolvedValue(result);
  const pending=waitForComputeResult("task",read);
  await new Promise((resolve)=>setTimeout(resolve,0)); listener(task("completed"));
  await expect(pending).resolves.toEqual(result);expect(read).toHaveBeenCalledOnce();expect(unlisten).toHaveBeenCalled();
});
it("detaching aborts the watcher while the durable task continues", async () => {
  vi.mocked(desktop.getBackgroundTask).mockResolvedValue(task("running"));const controller=new AbortController();
  const pending=waitForComputeResult("task",vi.fn(),controller.signal);
  await new Promise((resolve)=>setTimeout(resolve,0));controller.abort();
  await expect(pending).rejects.toMatchObject({name:"AbortError"});expect(unlisten).toHaveBeenCalled();
});

it("recovers a missed event subscription and reads only the submitted task", async () => {
  vi.mocked(desktop.listenSchedulerProgress).mockRejectedValueOnce(new Error("disconnected"));
  vi.mocked(desktop.getBackgroundTask).mockResolvedValue(task("completed"));
  await expect(waitForComputeResult("task",vi.fn().mockResolvedValue({ok:true}))).resolves.toEqual({ok:true});
  expect(desktop.getBackgroundTask).toHaveBeenCalledWith("task");
  expect(desktop.listBackgroundTasks).not.toHaveBeenCalled();
  expect(desktop.listenSchedulerProgress).toHaveBeenCalledTimes(2);
  expect(unlisten).toHaveBeenCalled();
});

it("an hour without completion does not fabricate a timeout failure",async()=>{
  vi.useFakeTimers();
  try {
    vi.mocked(desktop.getBackgroundTask).mockResolvedValue(task("running"));
    let finished=false;
    const pending=waitForComputeResult("task",vi.fn().mockResolvedValue({ok:true})).then(r=>{finished=true;return r;});
    await vi.advanceTimersByTimeAsync(3600000);
    expect(finished).toBe(false);
    expect(desktop.listBackgroundTasks).not.toHaveBeenCalled();
    vi.mocked(desktop.getBackgroundTask).mockResolvedValue(task("completed"));listener(task("completed"));
    await expect(pending).resolves.toEqual({ok:true});
  }finally{vi.useRealTimers();}
});
