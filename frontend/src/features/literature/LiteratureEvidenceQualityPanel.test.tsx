import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import LiteratureEvidenceQualityPanel from './LiteratureEvidenceQualityPanel';
import type {EvidenceQualityPreview,EvidenceQualityService} from '../../bridge/evidenceQualityTypes';

const preview:EvidenceQualityPreview={readOnly:true,reportSha256:'a'.repeat(64),report:{schemaVersion:'literature-evidence-quality.v1',algorithmVersion:'index-traceability.v1',projectId:'project-a',scope:'reviewed_numeric_evidence',sourceType:'literature',registrySha256:'b'.repeat(64),reviewedEvidenceCount:1,
  counts:{indexedReferenceMatched:1,missingIndexedSource:0,changedIndexedSource:0,doiNotRecorded:1,doiLinkMissing:0,locationNotRecorded:0,unknownField:0,incompatibleUnit:0},
  checks:[{evidenceSha256:'c'.repeat(64),artifactId:'evidence-a',documentId:'doc-a',versionId:'version-a',chunkId:'chunk-a',field:'property.hardness_hv',value:500,unit:'HV',indexedSourceState:'matched',observedIndexSha256:'d'.repeat(64),doiState:'not_recorded',locationRecorded:true,unitState:'normalized',canonicalField:'property.hardness_hv',canonicalUnit:'HV',normalizedValue:500,rawSourceBytesVerified:false,comparability:'conditions_not_recorded'}],limitations:['indexed_references_only','not_a_scientific_validity_score']}};
let service:EvidenceQualityService;
beforeEach(()=>{service={preview:vi.fn().mockResolvedValue(preview),archive:vi.fn().mockResolvedValue({artifactId:'report-a',contentSha256:preview.reportSha256,reportSha256:preview.reportSha256})};});
async function load(){render(<LiteratureEvidenceQualityPanel projectId="project-a" service={service}/>);expect(service.preview).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'核查项目证据'}));await screen.findByLabelText('证据核查预览');}
it('requires a project and explicit read-only preview before persisting the exact reviewed hash',async()=>{
  const empty=render(<LiteratureEvidenceQualityPanel projectId={null} service={service}/>);expect(screen.getByRole('button',{name:'核查项目证据'})).toBeDisabled();expect(service.preview).not.toHaveBeenCalled();empty.unmount();
  await load();expect(service.preview).toHaveBeenCalledWith('project-a');expect(screen.getByText('规范单位值 500 HV；此核查不评价实验条件，不作直接性能比较。')).toBeInTheDocument();
  expect(service.archive).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'保存本次核查报告'}));
  expect(await screen.findByRole('status')).toHaveTextContent('不可变成果 report-a');expect(service.archive).toHaveBeenCalledWith('project-a',preview.reportSha256);
});
it('invalidates a stale snapshot and does not claim a report was saved',async()=>{
  vi.mocked(service.archive).mockRejectedValue('literature_quality_preview_changed');await load();fireEvent.click(screen.getByRole('button',{name:'保存本次核查报告'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('已变化，请重新核查');expect(screen.queryByLabelText('证据核查预览')).not.toBeInTheDocument();expect(screen.queryByRole('status')).not.toBeInTheDocument();
});
it('rejects foreign or misleading claims and hides arbitrary backend error details',async()=>{
  vi.mocked(service.preview).mockResolvedValue({...preview,report:{...preview.report,projectId:'project-b'}});render(<LiteratureEvidenceQualityPanel projectId="project-a" service={service}/>);
  fireEvent.click(screen.getByRole('button',{name:'核查项目证据'}));expect(await screen.findByRole('alert')).toHaveTextContent('核查未完成');
  vi.mocked(service.preview).mockResolvedValue({...preview,report:{...preview.report,checks:[{...preview.report.checks[0],rawSourceBytesVerified:true}]}} as unknown as EvidenceQualityPreview);
  fireEvent.click(screen.getByRole('button',{name:'核查项目证据'}));await waitFor(()=>expect(service.preview).toHaveBeenCalledTimes(2));expect(screen.queryByLabelText('证据核查预览')).not.toBeInTheDocument();
  vi.mocked(service.preview).mockRejectedValue('synthetic-private-backend-detail');fireEvent.click(screen.getByRole('button',{name:'核查项目证据'}));await waitFor(()=>expect(service.preview).toHaveBeenCalledTimes(3));expect(screen.queryByText(/synthetic-private/)).not.toBeInTheDocument();expect(service.archive).not.toHaveBeenCalled();
});
it('does not convert an empty set of archived evidence into a report or a quality score',async()=>{
  vi.mocked(service.preview).mockResolvedValue({...preview,report:{...preview.report,reviewedEvidenceCount:0,checks:[],counts:{...preview.report.counts,indexedReferenceMatched:0,doiNotRecorded:0}}});
  await load();expect(screen.getByText(/尚无已归档证据/)).toBeInTheDocument();expect(screen.getByRole('button',{name:'保存本次核查报告'})).toBeDisabled();expect(service.archive).not.toHaveBeenCalled();
});
it('discards a late preview after switching the current project',async()=>{
  let release!:(value:EvidenceQualityPreview)=>void;vi.mocked(service.preview).mockImplementation(()=>new Promise(resolve=>{release=resolve;}));
  const node=render(<LiteratureEvidenceQualityPanel projectId="project-a" service={service}/>);fireEvent.click(screen.getByRole('button',{name:'核查项目证据'}));
  await waitFor(()=>expect(service.preview).toHaveBeenCalled());node.rerender(<LiteratureEvidenceQualityPanel projectId="project-b" service={service}/>);await act(async()=>release(preview));
  expect(screen.queryByLabelText('证据核查预览')).not.toBeInTheDocument();expect(service.archive).not.toHaveBeenCalled();expect(screen.getByRole('button',{name:'核查项目证据'})).toBeEnabled();
});
