import {render,screen,fireEvent} from "@testing-library/react";
import {expect,it} from "vitest";
import ResearchWorkflowGuide from "./ResearchWorkflowGuide";
import {useResearchStore} from "../../stores/researchStore";
it("guides offline startup with actual project counts and missing sources",()=>{
  useResearchStore.setState({currentProject:null});
  const project={id:"p",name:"合成课题",description:"",status:"active" as const,materialSystem:null,tags:[],createdAt:"now",updatedAt:"now"};
  render(<ResearchWorkflowGuide project={project} summary={{counts:{dataset:2,model:1},missingSources:1}}/>);
  fireEvent.click(screen.getByText("科研入门与离线流程"));
  expect(screen.getByText(/先使用“设为当前项目”/)).toBeVisible();
  expect(screen.getByText(/已关联 2 项/)).toBeVisible();
  expect(screen.getByRole("status")).toHaveTextContent("1 项来源缺失");
  expect(screen.getByRole("link",{name:"专业数据源"})).toHaveAttribute("href","/data?tab=sources");
});
