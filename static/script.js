const COLORS=['#00f0ff','#ff00e5','#00ff88','#ff8800','#aa66ff','#ffdd00','#ff6688','#66ddff'];
const TRAIL_COLORS=['#00f0ff88','#ff00e588','#00ff8888','#ff880088','#aa66ff88','#ffdd0088','#ff668888','#66ddff88'];
const WALL_COLOR='#2a2a4e';
const OBSTRUCTION_COLOR='#4a2a2e';
const BG_COLOR='#08080f';
const GRID_COLOR='#0f0f1a';

let currentGame=null;
const canvas=document.getElementById('gameCanvas');
const ctx=canvas.getContext('2d');

// Cell size adapts to grid
function cellSize(w,h){return Math.min(Math.floor(800/w),Math.floor(600/h),16)}

function renderGame(game){
  if(!game)return;
  currentGame=game;
  document.getElementById('game-view-card').style.display='block';
  const cs=cellSize(game.width,game.height);
  canvas.width=game.width*cs;
  canvas.height=game.height*cs;

  // Background
  ctx.fillStyle=BG_COLOR;
  ctx.fillRect(0,0,canvas.width,canvas.height);

  // Grid lines
  ctx.strokeStyle=GRID_COLOR;
  ctx.lineWidth=0.5;
  for(let x=0;x<=game.width;x++){ctx.beginPath();ctx.moveTo(x*cs,0);ctx.lineTo(x*cs,canvas.height);ctx.stroke()}
  for(let y=0;y<=game.height;y++){ctx.beginPath();ctx.moveTo(0,y*cs);ctx.lineTo(canvas.width,y*cs);ctx.stroke()}

  // Cells
  for(let y=0;y<game.height;y++){
    for(let x=0;x<game.width;x++){
      const cell=game.grid[y][x];
      if(cell===0)continue;
      if(cell===1){ctx.fillStyle=WALL_COLOR;ctx.fillRect(x*cs,y*cs,cs,cs)}
      else if(cell===2){ctx.fillStyle=OBSTRUCTION_COLOR;ctx.fillRect(x*cs,y*cs,cs,cs)}
      else{
        const pi=cell-3;
        ctx.fillStyle=TRAIL_COLORS[pi%8];
        ctx.fillRect(x*cs,y*cs,cs,cs);
      }
    }
  }

  // Player heads (bright glow)
  for(const p of game.players){
    if(!p.alive)continue;
    const color=COLORS[p.index%8];
    ctx.fillStyle=color;
    ctx.shadowColor=color;
    ctx.shadowBlur=cs*1.5;
    ctx.fillRect(p.x*cs,p.y*cs,cs,cs);
    ctx.shadowBlur=0;
    // Direction indicator
    ctx.fillStyle='#ffffff';
    const cx=p.x*cs+cs/2,cy=p.y*cs+cs/2,s=cs/4;
    ctx.beginPath();
    if(p.direction==='Up')ctx.moveTo(cx,cy-s),ctx.lineTo(cx-s,cy+s),ctx.lineTo(cx+s,cy+s);
    else if(p.direction==='Down')ctx.moveTo(cx,cy+s),ctx.lineTo(cx-s,cy-s),ctx.lineTo(cx+s,cy-s);
    else if(p.direction==='Left')ctx.moveTo(cx-s,cy),ctx.lineTo(cx+s,cy-s),ctx.lineTo(cx+s,cy+s);
    else ctx.moveTo(cx+s,cy),ctx.lineTo(cx-s,cy-s),ctx.lineTo(cx-s,cy+s);
    ctx.fill();
  }

  // Game info
  const info=document.getElementById('gameInfo');
  const statusText=game.status==='Running'?'⚡ RUNNING':game.status==='Finished'?'🏁 FINISHED':'⏳ WAITING';
  const alive=game.players.filter(p=>p.alive).length;
  info.innerHTML=`<span>${statusText}</span><span>Course: ${game.course_name} (Lv.${game.course_level})</span><span>Tick: ${game.tick}</span><span>Alive: ${alive}/${game.players.length}</span><span>Grid: ${game.width}×${game.height}</span>`;

  // Player list
  const pl=document.getElementById('playerList');
  pl.innerHTML=game.players.map((p,i)=>{
    const c=COLORS[i%8];
    const st=p.alive?'ALIVE':'CRASHED';
    const extra=game.winner===i?' 👑':'';
    return `<span class="player-tag" style="border-color:${c};color:${c}">${p.name}: ${st} (d:${p.distance})${extra}</span>`;
  }).join('');
}

// Fetch initial data
async function fetchGames(){
  try{
    const r=await fetch('/api/games');
    const data=await r.json();
    renderActiveGames(data.active||[]);
    renderFinishedGames(data.finished||[]);
    // Auto-show first active game
    if(data.active&&data.active.length>0)renderGame(data.active[0]);
  }catch(e){console.error('Fetch games error:',e)}
}
async function fetchLeaderboard(){
  try{
    const r=await fetch('/api/leaderboard');
    const data=await r.json();
    renderLeaderboard(data);
  }catch(e){console.error('Fetch leaderboard error:',e)}
}

function renderActiveGames(games){
  const el=document.getElementById('activeGames');
  if(!games.length){el.innerHTML='<div class="no-data">No active games. Waiting for LLM players to connect...</div>';return}
  el.innerHTML=games.map(g=>{
    const alive=g.players.filter(p=>p.alive).length;
    const names=g.players.map(p=>p.name).join(', ');
    const st=g.status==='Running'?'running':'waiting';
    return `<div class="game-item" onclick='renderGame(${JSON.stringify(g).replace(/'/g,"&#39;")})'>
      <div><strong>${g.course_name}</strong> (Lv.${g.course_level})<br><small style="color:var(--text-dim)">${names}</small></div>
      <div style="text-align:right"><span class="status ${st}">${g.status}</span><br><small>${alive}/${g.players.length} alive · tick ${g.tick}</small></div>
    </div>`;
  }).join('');
}

function renderFinishedGames(games){
  const el=document.getElementById('finishedGames');
  if(!games.length){el.innerHTML='<div class="no-data">No finished games yet.</div>';return}
  const recent=games.slice(-20).reverse();
  el.innerHTML=recent.map(g=>{
    const winner=g.winner!==null&&g.winner!==undefined&&g.players[g.winner]?g.players[g.winner].name:'Draw';
    const names=g.players.map(p=>p.name).join(', ');
    return `<div class="game-item" onclick='renderGame(${JSON.stringify(g).replace(/'/g,"&#39;")})'>
      <div><strong>${g.course_name}</strong> (Lv.${g.course_level})<br><small style="color:var(--text-dim)">${names}</small></div>
      <div style="text-align:right"><span class="status finished">FINISHED</span><br><small>Winner: ${winner}</small></div>
    </div>`;
  }).join('');
}

function renderLeaderboard(entries){
  const el=document.getElementById('leaderboard');
  if(!entries.length){el.innerHTML='<div class="no-data">No games played yet.</div>';return}
  el.innerHTML=`<table class="lb-table">
    <tr><th>#</th><th>PLAYER</th><th>WINS</th><th>POINTS</th><th>GAMES</th><th>LEVEL</th></tr>
    ${entries.map((e,i)=>`<tr>
      <td class="rank">${i+1}</td><td>${e.name}</td><td>${e.wins}</td>
      <td>${e.total_points}</td><td>${e.games_played}</td><td>${e.highest_level}</td>
    </tr>`).join('')}
  </table>`;
}

// SSE for real-time updates
function connectSSE(){
  const es=new EventSource('/api/stream');
  es.onmessage=(e)=> {
    try{
      const msg=JSON.parse(e.data);
      if(msg.type==='game_update'){
        renderGame(msg.game);
        fetchGames();
      }else if(msg.type==='game_finished'){
        fetchGames();
        fetchLeaderboard();
        if(msg.game)renderGame(msg.game);
      }else if(msg.type==='game_started'){
        fetchGames();
      }
    }catch(err){console.error('SSE parse error:',err)}
  };
  es.onerror=()=> {
    console.warn('SSE disconnected, reconnecting in 3s...');
    es.close();
    setTimeout(connectSSE,3000);
  };
}

// Init
fetchGames();
fetchLeaderboard();
connectSSE();
// Periodic refresh
setInterval(()=>{fetchGames();fetchLeaderboard()},5000);
