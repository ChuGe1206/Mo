// Apply only to an inspected file using the persisted STATE IDs.
// TARGET_PAGE: "components" or "screens"; one page switch per call.
const page=await figma.getNodeByIdAsync(STATE.pages[TARGET_PAGE]);
await figma.setCurrentPageAsync(page);
await Promise.all(STATE.font.styles.map(style=>figma.loadFontAsync({family:STATE.font.family,style})));
const changed=[],created=[];
if(TARGET_PAGE==="components"){
 for(const [name,id] of Object.entries(STATE.components)){
  const c=await figma.getNodeByIdAsync(id);
  for(const n of c.findAllWithCriteria({types:["FRAME"]})){
   let width=null;
   if(name.startsWith("Bubble/")&&["Preedit","Footer"].includes(n.name)){width=c.width-24;}
   if(name.startsWith("SkinCard/")){width=({"Caption":228,"Mini candidates":204,"Selected":90})[n.name]||null;}
   if(name.startsWith("Nav/")&&n.name==="Mark"){width=20;}
   if(width){n.resize(width,n.height);n.primaryAxisSizingMode="FIXED";n.counterAxisSizingMode="AUTO";changed.push(n.id);}
  }
  if(name.startsWith("Bubble/Bamboo/")&&!c.findOne(n=>n.name==="Original bamboo motif")){
   const svg=figma.createNodeFromSvg('<svg width="64" height="34" viewBox="0 0 64 34" xmlns="http://www.w3.org/2000/svg" fill="none"><path d="M12 33 30 5 M21 21Q12 12 3 15Q13 23 21 21 M26 13Q36 4 48 7Q37 15 26 13 M18 27Q31 17 44 21Q28 31 18 27" stroke="#526143" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>');
   c.appendChild(svg);svg.name="Original bamboo motif";svg.layoutPositioning="ABSOLUTE";svg.x=c.width/2-32;svg.y=1;svg.opacity=0.18;
   created.push(svg.id,...svg.findAll(()=>true).map(n=>n.id));changed.push(c.id);
  }
 }
}else if(TARGET_PAGE==="screens"){
 const widths={"Skin gallery row":840,"Live candidate preview":840,"Privacy summary":840,"背景颜色":284,"强调颜色":284,"文字颜色":284,"Custom skin actions":840,"Mo wordmark":180};
 for(const n of page.findAllWithCriteria({types:["FRAME"]})){
  if(widths[n.name]&&n.layoutMode==="HORIZONTAL"){n.resize(widths[n.name],n.height);n.primaryAxisSizingMode="FIXED";n.counterAxisSizingMode="AUTO";changed.push(n.id);}
 }
 for(const n of page.findAllWithCriteria({types:["INSTANCE"]})){
  const k=Object.keys(n.componentProperties).find(p=>p.startsWith("Value#"));
  if(k&&n.componentProperties[k].value==="横排候选"){n.setProperties({[k]:"竖排候选"});changed.push(n.id);}
 }
}else{throw new Error("Unknown TARGET_PAGE");}
return {createdNodeIds:created,mutatedNodeIds:changed};
