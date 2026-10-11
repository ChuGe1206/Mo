// Run only the selected page branch. Inspect the ledger before any replay.
await figma.loadFontAsync({family:"Noto Sans SC",style:"Regular"});
await figma.loadFontAsync({family:"Noto Sans SC",style:"Medium"});
await figma.loadFontAsync({family:"Noto Sans SC",style:"Bold"});
await figma.loadFontAsync({family:"Noto Emoji",style:"Regular"});
if(TARGET_PAGE==="candidates"){
 const p=await figma.getNodeByIdAsync("2:3");await figma.setCurrentPageAsync(p);
 figma.skipInvisibleInstanceChildren=false;
 const grid=await figma.getNodeByIdAsync("I68:3306;75:814");grid.visible=true;
 return {mutatedNodeIds:[grid.id]};
}
if(TARGET_PAGE==="icons"){
 const p=await figma.getNodeByIdAsync("73:388");await figma.setCurrentPageAsync(p);
 const hero=await figma.getNodeByIdAsync("73:405");
 hero.setBoundVariable("itemSpacing",await figma.variables.getVariableByIdAsync("VariableID:2:102"));
 return {mutatedNodeIds:[hero.id]};
}
throw new Error("Select candidates or icons");
