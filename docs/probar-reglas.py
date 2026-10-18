"""Prueba firebase-reglas.json en el emulador oficial de Firebase, antes de publicarlas.

Cada paso escribe como lo hace Parabellum (actualizaciones de varias rutas a la vez) con un usuario
falso y compara con lo esperado (permitido o no).

Cómo:
  1. Baja el emulador (una vez, 34 MB, de Google):
     https://storage.googleapis.com/firebase-preview-drop/emulator/firebase-database-emulator-v4.11.2.jar
  2. java -jar firebase-database-emulator-v4.11.2.jar --host 127.0.0.1 --port 9399
  3. python docs/probar-reglas.py   (al final dice «TODO BIEN» o cuántas salieron mal)
"""
import base64
import json
import os
import sys
sys.stdout.reconfigure(encoding='utf-8')
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:9399"
NS = "parabellum-prueba"
RULES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "firebase-reglas.json")


def token(uid):
    if uid == "owner":
        return "owner"
    b64 = lambda d: base64.urlsafe_b64encode(json.dumps(d).encode()).decode().rstrip("=")
    payload = {"sub": uid, "user_id": uid, "uid": uid, "iat": 1700000000, "exp": 4102444800, "aud": NS, "iss": "https://securetoken.google.com/" + NS,
               "firebase": {"sign_in_provider": "custom", "identities": {}}}
    return b64({"alg": "none", "typ": "JWT"}) + "." + b64(payload) + "."


def req(method, path, uid, body=None):
    # El emulador toma «Authorization: Bearer» como administrador; el usuario va en ?auth= (como Parabellum).
    url = f"{BASE}/{path}.json?ns={NS}" + ("" if uid == "owner" else "&auth=" + token(uid))
    data = None if body is None else json.dumps(body).encode()
    headers = {"Content-Type": "application/json"} | ({"Authorization": "Bearer owner"} if uid == "owner" else {})
    r = urllib.request.Request(url, data=data, method=method, headers=headers)
    try:
        with urllib.request.urlopen(r) as resp:
            return resp.status, json.loads(resp.read() or b"null")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


fails = 0


def check(name, expected, method, path, uid, body=None):
    global fails
    status, _ = req(method, path, uid, body)
    ok = (status == 200) == expected
    if not ok:
        fails += 1
    print(f"{'OK ' if ok else 'MAL'} {name}: {'permitido' if status == 200 else f'negado ({status})'}")


# Reglas y datos de partida (como administrador).
rules = open(RULES, encoding="utf-8").read()
r = urllib.request.Request(f"{BASE}/.settings/rules.json?ns={NS}", data=rules.encode(), method="PUT", headers={"Authorization": "Bearer owner"})
print("reglas:", urllib.request.urlopen(r).status)
friends = {}
for a, b in [("A", "B"), ("A", "C"), ("A", "D"), ("B", "C"), ("B", "D")]:
    friends.setdefault(a, {})[b] = True
    friends.setdefault(b, {})[a] = True
req("PUT", "", "owner", {
    "friends": friends,
    # Un grupo de antes de la 0.27 (sin dueño).
    "chats": {"g_viejo": {"members": {"A": True, "B": True}, "meta": {"kind": "grupo", "name": "Viejo", "created": 1, "lastText": "", "lastName": "", "lastUid": "", "lastAt": 1}}},
    "userChats": {"A": {"g_viejo": True}, "B": {"g_viejo": True}},
})
TS = {".sv": "timestamp"}

# ---------- Crear un grupo (0.27: con dueño) y uno como lo crea la 0.26
check("A crea un grupo con dueño", True, "PATCH", "", "A", {
    "chats/g1/members/A": True, "chats/g1/members/B": True, "chats/g1/members/C": True,
    "userChats/A/g1": True, "userChats/B/g1": True, "userChats/C/g1": True,
    "chats/g1/meta": {"kind": "grupo", "name": "Los panas", "owner": "A", "created": TS, "lastText": "", "lastName": "", "lastUid": "", "lastAt": TS},
})
check("B crea un grupo como la 0.26 (sin dueño)", True, "PATCH", "", "B", {
    "chats/g2/members/B": True, "chats/g2/members/C": True, "userChats/B/g2": True, "userChats/C/g2": True,
    "chats/g2/meta": {"kind": "grupo", "name": "Otro", "created": TS, "lastText": "", "lastName": "", "lastUid": "", "lastAt": TS},
})
check("C crea un grupo poniendo de dueño a alguien de fuera", False, "PATCH", "", "C", {
    "chats/g3/members/C": True, "chats/g3/members/A": True, "userChats/C/g3": True, "userChats/A/g3": True,
    "chats/g3/meta": {"kind": "grupo", "name": "X", "owner": "D", "created": TS, "lastText": "", "lastName": "", "lastUid": "", "lastAt": TS},
})

# ---------- Editar: solo el dueño
check("B cambia el nombre (no es dueño)", False, "PUT", "chats/g1/meta/name", "B", "Hackeado")
check("A cambia el nombre (dueño)", True, "PUT", "chats/g1/meta/name", "A", "Los panas 2")
check("B pone foto (no es dueño)", False, "PUT", "chats/g1/meta/photo", "B", "data:image/jpeg;base64,AAAA")
check("A pone foto", True, "PUT", "chats/g1/meta/photo", "A", "data:image/jpeg;base64,AAAA")
check("A pone una foto enorme", False, "PUT", "chats/g1/meta/photo", "A", "data:image/jpeg;base64," + "A" * 40000)
check("A pone algo que no es imagen de foto", False, "PUT", "chats/g1/meta/photo", "A", "https://x/y.png")
check("B quita la foto (no es dueño)", False, "DELETE", "chats/g1/meta/photo", "B")
check("B borra el dueño", False, "DELETE", "chats/g1/meta/owner", "B")
check("B se pone de dueño", False, "PUT", "chats/g1/meta/owner", "B", "B")
check("B reescribe toda la meta sin dueño", False, "PUT", "chats/g1/meta", "B", {"kind": "grupo", "name": "Mío"})
check("B agrega a D (no es dueño)", False, "PATCH", "", "B", {"chats/g1/members/D": True, "userChats/D/g1": True})
check("A agrega a D", True, "PATCH", "", "A", {"chats/g1/members/D": True, "userChats/D/g1": True})
check("B saca a D (no es dueño)", False, "PATCH", "", "B", {"chats/g1/members/D": None, "userChats/D/g1": None})
check("A saca a C", True, "PATCH", "", "A", {"chats/g1/members/C": None, "userChats/C/g1": None})
check("A le quita la conversación a D sin sacarlo", False, "DELETE", "userChats/D/g1", "A")

# ---------- Mandar mensajes (todos los miembros), con respuesta
msg = {"from": "B", "name": "Bea", "text": "hola", "at": TS}
check("B manda un mensaje (como la 0.26)", True, "PATCH", "", "B", {
    "messages/g1/m1": msg, "chats/g1/meta/lastText": "hola", "chats/g1/meta/lastName": "Bea", "chats/g1/meta/lastUid": "B", "chats/g1/meta/lastAt": TS,
    "userChats/A/g1": True, "userChats/B/g1": True, "userChats/D/g1": True,
})
check("C (ya fuera) manda un mensaje", False, "PATCH", "", "C", {"messages/g1/mc": {"from": "C", "name": "C", "text": "x", "at": TS}, "chats/g1/meta/lastText": "x"})
check("A responde a m1", True, "PATCH", "", "A", {
    "messages/g1/m2": {"from": "A", "name": "Ana", "text": "qué tal", "at": TS, "reply": {"id": "m1", "name": "Bea", "text": "hola"}},
    "chats/g1/meta/lastText": "qué tal", "chats/g1/meta/lastName": "Ana", "chats/g1/meta/lastUid": "A", "chats/g1/meta/lastAt": TS,
})
check("respuesta con un campo de más", False, "PUT", "messages/g1/m3", "A", {"from": "A", "name": "Ana", "text": "x", "at": 5, "reply": {"id": "m1", "name": "Bea", "text": "hola", "otro": 1}})
check("respuesta incompleta", False, "PUT", "messages/g1/m3", "A", {"from": "A", "name": "Ana", "text": "x", "at": 5, "reply": {"id": "m1"}})
check("A edita su respuesta (la cita se queda)", True, "PUT", "messages/g1/m2", "A", {"from": "A", "name": "Ana", "text": "qué tal?", "at": req("GET", "messages/g1/m2/at", "A")[1], "edited": TS, "reply": {"id": "m1", "name": "Bea", "text": "hola"}})

# ---------- Reacciones
check("D reacciona al mensaje de A", True, "PUT", "reactions/g1/m2/D", "D", "👍")
check("B reacciona con ❤️", True, "PUT", "reactions/g1/m2/B", "B", "❤️")
check("B cambia su reacción", True, "PUT", "reactions/g1/m2/B", "B", "😂")
check("B quita su reacción", True, "DELETE", "reactions/g1/m2/B", "B")
check("B reacciona por D", False, "PUT", "reactions/g1/m2/D", "B", "🔥")
check("C (fuera del grupo) reacciona", False, "PUT", "reactions/g1/m2/C", "C", "👍")
check("reacción a un mensaje que no existe", False, "PUT", "reactions/g1/nada/B", "B", "👍")
check("reacción muy larga", False, "PUT", "reactions/g1/m2/B", "B", "x" * 17)
check("B borra las reacciones del mensaje de A", False, "DELETE", "reactions/g1/m2", "B")
check("D lee las reacciones", True, "GET", "reactions/g1", "D")
check("C (fuera) lee las reacciones", False, "GET", "reactions/g1", "C")
check("A borra su mensaje con sus reacciones", True, "PATCH", "", "A", {"messages/g1/m2": None, "reactions/g1/m2": None})

# ---------- Grupo de antes (sin dueño): el primero que lo edita queda como dueño
check("B renombra el grupo viejo y queda como dueño", True, "PATCH", "", "B", {"chats/g_viejo/meta/name": "Viejo 2", "chats/g_viejo/meta/owner": "B"})
check("A ya no puede renombrarlo", False, "PUT", "chats/g_viejo/meta/name", "A", "Mío")
check("A no se puede poner de dueño", False, "PUT", "chats/g_viejo/meta/owner", "A", "A")

# ---------- El dueño sale y deja el grupo a otro
check("A sale de g1 y se lo deja a B", True, "PATCH", "", "A", {"chats/g1/members/A": None, "userChats/A/g1": None, "chats/g1/meta/owner": "B"})
check("B ya puede renombrarlo", True, "PUT", "chats/g1/meta/name", "B", "De B")
check("D sigue sin poder", False, "PUT", "chats/g1/meta/name", "D", "De D")
check("B no puede dejárselo a alguien de fuera", False, "PUT", "chats/g1/meta/owner", "B", "C")

# ---------- El dueño sale como la 0.26 (sin pasarlo): otro miembro lo toma
check("B sale de g_viejo como la 0.26", True, "PATCH", "", "B", {"chats/g_viejo/members/B": None, "userChats/B/g_viejo": None})
check("A toma el grupo sin dueño", True, "PATCH", "", "A", {"chats/g_viejo/meta/name": "Rescatado", "chats/g_viejo/meta/owner": "A"})

# ---------- Conversación directa (sin dueño): como siempre
direct = {"chats/d_A_D/members/A": True, "chats/d_A_D/members/D": True, "chats/d_A_D/meta/kind": "directo", "userChats/A/d_A_D": True, "userChats/D/d_A_D": True}
check("A abre conversación con D", True, "PATCH", "", "A", direct)
check("A la vuelve a abrir", True, "PATCH", "", "A", direct)
check("D le escribe a A", True, "PATCH", "", "D", {"messages/d_A_D/x1": {"from": "D", "name": "Dani", "text": "hey", "at": TS}, "chats/d_A_D/meta/lastText": "hey", "chats/d_A_D/meta/lastName": "Dani", "chats/d_A_D/meta/lastUid": "D", "chats/d_A_D/meta/lastAt": TS, "userChats/A/d_A_D": True, "userChats/D/d_A_D": True})
check("B (fuera) se mete a la conversación de A y D", False, "PUT", "chats/d_A_D/members/B", "B", True)
check("C abre conversación con alguien que no es su amigo", False, "PATCH", "", "C", {"chats/d_C_D/members/C": True, "chats/d_C_D/members/D": True, "chats/d_C_D/meta/kind": "directo", "userChats/C/d_C_D": True, "userChats/D/d_C_D": True})

# ---------- Comunidades (docs/plan-diciembre.md, sección 6)
import time
NOW = int(time.time() * 1000)
CID = "c0000000000a1"
member = lambda role, name, **extra: {"role": role, "name": name, "joinedAt": TS} | extra
check("A crea su comunidad (dueño)", True, "PATCH", "", "A", {
    f"communities/{CID}": {"name": "Parabellum", "description": "El servidor", "owner": "A", "createdAt": TS},
    f"communityMembers/{CID}/A": member("owner", "Juan"), f"userCommunities/A/{CID}": True})
check("B no se pone como dueño de la de A", False, "PUT", f"communityMembers/{CID}/B", "B", member("owner", "B"))
check("B no crea una comunidad a nombre de A", False, "PUT", "communities/c0000000000b2", "B", {"name": "Falsa", "owner": "A", "createdAt": TS})
check("B no entra sin código", False, "PUT", f"communityMembers/{CID}/B", "B", member("member", "B"))
check("B no entra con un código inventado", False, "PUT", f"communityMembers/{CID}/B", "B", member("member", "B", invite="ZZZZZZZZ"))
check("B no crea códigos de la comunidad de A", False, "PATCH", "", "B", {"inviteCodes/BBBBBBBB": {"cid": CID, "createdBy": "B", "expiresAt": NOW + 86400000}})
check("A crea un código (1 día)", True, "PATCH", "", "A", {"inviteCodes/ABCDEFGH": {"cid": CID, "createdBy": "A", "expiresAt": NOW + 86400000}, f"communityInvites/{CID}/ABCDEFGH": NOW + 86400000})
check("A no crea un código de 2 meses", False, "PUT", "inviteCodes/ABCDEFGJ", "A", {"cid": CID, "createdBy": "A", "expiresAt": NOW + 60 * 86400000})
check("B no entra como administrador con el código", False, "PUT", f"communityMembers/{CID}/B", "B", member("admin", "B", invite="ABCDEFGH"))
check("B no entra con permisos con el código", False, "PUT", f"communityMembers/{CID}/B", "B", member("member", "B", invite="ABCDEFGH", caps={"support": True}))
check("B entra con el código", True, "PATCH", "", "B", {f"communityMembers/{CID}/B": member("member", "B", invite="ABCDEFGH"), f"userCommunities/B/{CID}": True})
check("C entra con el código", True, "PATCH", "", "C", {f"communityMembers/{CID}/C": member("member", "C", invite="ABCDEFGH"), f"userCommunities/C/{CID}": True})
req("PUT", "inviteCodes/VENCD234", "owner", {"cid": CID, "createdBy": "A", "expiresAt": NOW - 1000})
check("D no entra con un código vencido", False, "PUT", f"communityMembers/{CID}/D", "D", member("member", "D", invite="VENCD234"))
check("D (fuera) ve lo público de la comunidad", True, "GET", f"communities/{CID}", "D")
check("D (fuera) no ve los miembros", False, "GET", f"communityMembers/{CID}", "D")
check("D (fuera) no ve los anuncios", False, "GET", f"communityPosts/{CID}", "D")
check("D lee un código que conoce", True, "GET", "inviteCodes/ABCDEFGH", "D")
check("D no puede listar los códigos", False, "GET", "inviteCodes", "D")
check("B (miembro) no ve los códigos de la comunidad", False, "GET", f"communityInvites/{CID}", "B")
check("D no se agrega la comunidad a su lista sin ser miembro", False, "PUT", f"userCommunities/D/{CID}", "D", True)
check("B no se da permisos a sí mismo", False, "PUT", f"communityMembers/{CID}/B/caps", "B", {"support": True})
check("B no se hace administrador", False, "PUT", f"communityMembers/{CID}/B", "B", member("admin", "B", invite="ABCDEFGH"))
check("A le da soporte a B", True, "PUT", f"communityMembers/{CID}/B", "A", member("member", "B", invite="ABCDEFGH", caps={"support": True}))
check("A hace administrador a B", True, "PUT", f"communityMembers/{CID}/B/role", "A", "admin")
check("B (admin) no hace administrador a C", False, "PUT", f"communityMembers/{CID}/C/role", "B", "admin")
check("B (admin) le da publicar a C", True, "PUT", f"communityMembers/{CID}/C/caps", "B", {"publish": True})
check("B (admin) no se cambia sus permisos", False, "PUT", f"communityMembers/{CID}/B/caps", "B", {"support": True, "publish": True, "servers": True})
check("B (admin) no saca al dueño", False, "DELETE", f"communityMembers/{CID}/A", "B")
check("B (admin) no hace dueño a nadie", False, "PUT", f"communityMembers/{CID}/C/role", "B", "owner")
check("C (miembro) no publica anuncios", False, "PUT", f"communityPosts/{CID}/p1", "C", {"title": "Hola", "text": "x", "author": "C", "at": TS})
check("B (admin) publica un anuncio", True, "PUT", f"communityPosts/{CID}/p1", "B", {"title": "Abrimos", "text": "El 1 de diciembre", "author": "B", "authorName": "B", "at": TS})
check("B no publica a nombre de A", False, "PUT", f"communityPosts/{CID}/p2", "B", {"title": "Falso", "text": "x", "author": "A", "at": TS})
check("C (miembro) lee los anuncios", True, "GET", f"communityPosts/{CID}", "C")
check("C (publicar) no edita los servidores", False, "PUT", f"communityServers/{CID}/v00000001", "C", {"name": "Principal", "address": "mc.ejemplo.com"})
check("C (publicar) sube el paquete oficial", True, "PUT", f"communityPackages/{CID}/p00000001", "C", {"name": "Oficial", "catalogId": "parabellum-oficial", "version": "1.0.0", "minecraft": "26.3", "loader": "Fabric", "at": TS})
check("A edita los servidores", True, "PUT", f"communityServers/{CID}/v00000001", "A", {"name": "Principal", "address": "mc.ejemplo.com", "bedrock": "mc.ejemplo.com:19132", "package": "p00000001"})
check("C (miembro) no cambia las reglas", False, "PUT", f"communityRules/{CID}", "C", {"text": "nada", "at": TS, "by": "C"})
check("A escribe las reglas", True, "PUT", f"communityRules/{CID}", "A", {"text": "Respeto", "at": TS, "by": "A"})
check("B (admin) cambia la descripción", True, "PUT", f"communities/{CID}/description", "B", "Nueva")
check("B (admin) no cambia el dueño", False, "PUT", f"communities/{CID}/owner", "B", "B")
check("C (miembro) no cambia el nombre", False, "PUT", f"communities/{CID}/name", "C", "Mía")
check("C sale de la comunidad", True, "PATCH", "", "C", {f"communityMembers/{CID}/C": None, f"userCommunities/C/{CID}": None})
check("A (dueño) no puede salir", False, "DELETE", f"communityMembers/{CID}/A", "A")
check("A saca a B y lo quita de su lista", True, "PATCH", "", "A", {f"communityMembers/{CID}/B": None, f"userCommunities/B/{CID}": None})
check("B (fuera) ya no ve los anuncios", False, "GET", f"communityPosts/{CID}", "B")

print("\nTODO BIEN" if not fails else f"\n{fails} MAL")
sys.exit(1 if fails else 0)
