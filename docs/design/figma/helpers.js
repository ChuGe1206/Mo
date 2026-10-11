// Mo UI helpers. All measurements are device-independent design pixels.
const made=[];
function keep(n){made.push(n);return n;}
const vars={};
for(const [name,id] of Object.entries(STATE.variables)){vars[name]=await figma.variables.getVariableByIdAsync(id);}
await Promise.all(STATE.font.styles.map(style=>figma.loadFontAsync({family:STATE.font.family,style})));
function paint(theme,role){return figma.variables.setBoundVariableForPaint({type:"SOLID",color:{r:0,g:0,b:0}},"color",vars["color/"+theme.toLowerCase()+"/"+role]);}
function fill(n,theme,role){n.fills=[paint(theme,role)];}
function round(n,r){for(const field of ["topLeftRadius","topRightRadius","bottomLeftRadius","bottomRightRadius"]){n.setBoundVariable(field,vars["radius/"+r]);}}
function gap(n,v){n.setBoundVariable("itemSpacing",vars["spacing/"+v]);}
function pad(n,v){for(const field of ["paddingLeft","paddingRight","paddingTop","paddingBottom"]){n.setBoundVariable(field,vars["spacing/"+v]);}}
function box(parent,name,direction,width,padding=0,space=0,theme="Paper",role=null){
 const n=keep(figma.createAutoLayout(direction));n.name=name;n.fills=[];n.clipsContent=false;n.counterAxisAlignItems="MIN";
 if(width){n.resize(width,1);n.primaryAxisSizingMode=direction==="VERTICAL"?"AUTO":"FIXED";n.counterAxisSizingMode=direction==="VERTICAL"?"FIXED":"AUTO";}
 else {n.primaryAxisSizingMode="AUTO";n.counterAxisSizingMode="AUTO";}
 pad(n,padding);gap(n,space);parent.appendChild(n);
 if(role){fill(n,theme,role);}
 return n;
}
async function txt(parent,name,value,style="Body",theme="Paper",role="text",width=null){
 const n=keep(figma.createText());n.name=name;await n.setTextStyleIdAsync(STATE.styles[style]);n.characters=value;
 n.textAutoResize=width?"HEIGHT":"WIDTH_AND_HEIGHT";
 if(width){n.resize(width,n.height);n.textAutoResize="HEIGHT";}
 n.fills=[paint(theme,role)];parent.appendChild(n);return n;
}
function icon(parent,name,theme="Paper",size=20){
 const paths={
 input:"M4 6h16v12H4z M7 9h1 M11 9h1 M15 9h1 M7 12h1 M11 12h1 M15 12h1 M8 15h8",
 brush:"m5 17 9-9 3 3-9 9H5z M14 8l3-3 3 3-3 3",
 shield:"M12 3 20 6v6c0 5-8 9-8 9S4 17 4 12V6z M8 12l3 3 5-6",
 chevron:"m9 6 6 6-6 6",close:"m6 6 12 12 M18 6 6 18",
 gear:"M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1z",
 check:"m5 12 4 4L19 6",back:"m14 6-6 6 6 6",add:"M12 5v14 M5 12h14",
 };
 const n=keep(figma.createNodeFromSvg('<svg width="'+size+'" height="'+size+'" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="'+paths[name]+'" stroke="#25352F" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>'));
 n.name="Icon/"+name;parent.appendChild(n);
 for(const vector of n.findAllWithCriteria({types:["VECTOR"]})){vector.strokes=[paint(theme,"muted")];}
 return n;
}
function allMadeIds(){
 return [...new Set(made.flatMap(n=>[n.id,...("findAll" in n?n.findAll(()=>true).map(c=>c.id):[])]))];
}
function expose(c,n,key,value){
 const id=c.addComponentProperty(key,"TEXT",value);n.componentPropertyReferences={characters:id};return id;
}
function inst(comp,parent,props={}){
 const n=keep(comp.createInstance());parent.appendChild(n);
 const keys=Object.keys(n.componentProperties);
 for(const [name,value] of Object.entries(props)){
  const key=keys.find(k=>k===name||k.startsWith(name+"#"));
  if(!key){throw new Error("Missing property "+name+" on "+n.name);}
  n.setProperties({[key]:value});
 }
 return n;
}
function combine(components,parent,name,cols,x,y){
 const set=keep(figma.combineAsVariants(components,parent));set.name=name;
 let maxX=0,maxY=0;
 set.children.forEach((c,i)=>{c.x=24+(i%cols)*(components[0].width+24);c.y=24+Math.floor(i/cols)*(Math.max(...components.map(v=>v.height))+24);maxX=Math.max(maxX,c.x+c.width);maxY=Math.max(maxY,c.y+c.height);});
 set.resizeWithoutConstraints(maxX+24,maxY+24);set.x=x;set.y=y;
 return set;
}
