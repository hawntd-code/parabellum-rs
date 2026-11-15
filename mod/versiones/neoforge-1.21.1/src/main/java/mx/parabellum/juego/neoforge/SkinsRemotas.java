package mx.parabellum.juego.neoforge;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.properties.Property;
import mx.parabellum.juego.Juego;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.resources.PlayerSkin;

import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/**
 * Usa la textura firmada de Parabellum cuando el cliente recibe un perfil de un servidor dedicado.
 *
 * <p>Cada consulta le cuesta a Parabellum una o dos lecturas de Firebase por jugador, así que no se sondea: se
 * pregunta por un jugador cuando aparece en la lista (los que entran juntos, como al conectarse, van en una sola
 * consulta) y después solo de vez en cuando, por si cambió de skin. Mirar la lista cada segundo no usa la red.
 */
public final class SkinsRemotas {
    private record Entry(long expires, CompletableFuture<PlayerSkin> texture) {}
    private static final Pattern NAME = Pattern.compile("[A-Za-z0-9_]{1,16}");
    /** Cada cuánto se mira si hay jugadores nuevos en la lista (solo en memoria). */
    private static final long CHECK = TimeUnit.SECONDS.toNanos(1);
    /** Se espera un poco tras ver al primero para juntar a los que entran a la vez. */
    private static final long GATHER = TimeUnit.MILLISECONDS.toNanos(1500);
    /** Cada cuánto se vuelve a preguntar por un jugador que ya se conoce, por si cambió de skin. */
    private static final long WITH_SKIN = TimeUnit.MINUTES.toNanos(15);
    private static final long WITHOUT_SKIN = TimeUnit.MINUTES.toNanos(10);
    /** Si Parabellum no contesta, no se insiste enseguida. */
    private static final long RETRY = TimeUnit.SECONDS.toNanos(30);
    /** Una respuesta que nunca llegó no bloquea las siguientes. */
    private static final long LOST = TimeUnit.SECONDS.toNanos(60);
    private static final int BATCH = 24;
    private static final Map<UUID, Entry> ENTRIES = new ConcurrentHashMap<>();
    private static ClientPacketListener connection;
    private static long nextCheck = System.nanoTime();
    private static long waitingSince;
    private static long retryAt;
    private static long requestedAt;
    private static boolean pending;

    private SkinsRemotas() {}

    static void tick() {
        Minecraft mc = Minecraft.getInstance();
        ClientPacketListener current = mc.isLocalServer() ? null : mc.getConnection();
        long now = System.nanoTime();
        if (current != connection) {
            connection = current;
            ENTRIES.clear();
            nextCheck = now;
            waitingSince = 0;
            retryAt = now;
            pending = false;
        }
        if (pending && now - requestedAt > LOST) pending = false;
        Juego juego = Juego.actual();
        if (current == null || juego == null || pending || now - nextCheck < 0) return;
        nextCheck = now + CHECK;
        ENTRIES.keySet().removeIf(uuid -> current.getPlayerInfo(uuid) == null);
        var profiles = new LinkedHashMap<UUID, String>();
        for (PlayerInfo info : current.getOnlinePlayers()) {
            GameProfile player = info.getProfile();
            Entry entry = ENTRIES.get(player.getId());
            if (entry != null && now - entry.expires() < 0) continue;
            if (!NAME.matcher(player.getName()).matches()) continue;
            profiles.put(player.getId(), player.getName());
            if (profiles.size() == BATCH) break;
        }
        if (profiles.isEmpty()) {
            waitingSince = 0;
            return;
        }
        if (waitingSince == 0) waitingSince = now;
        if (now - waitingSince < GATHER || now - retryAt < 0) return;
        JsonArray names = new JsonArray();
        profiles.values().forEach(names::add);
        requestedAt = now;
        pending = juego.pedirSkins(names, data -> received(current, profiles, data));
        // Una línea por consulta en latest.log: así se ve cada cuánto pregunta (al entrar alguien y cada 10-15 min).
        if (pending) System.out.println("[Parabellum] Skins: consulta por " + names.size() + " jugador(es) de la lista");
        else retryAt = now + RETRY;
        waitingSince = 0;
    }

    private static void received(ClientPacketListener request, Map<UUID, String> profiles, JsonObject data) {
        if (request != connection) return;
        pending = false;
        long checked = System.nanoTime();
        if (data == null) {
            // Parabellum cerrado o sin conexión: se vuelve a intentar más tarde, sin dar a nadie por «sin skin».
            retryAt = checked + RETRY;
            return;
        }
        for (var player : profiles.entrySet()) {
            UUID id = player.getKey();
            String name = player.getValue();
            if (connection.getPlayerInfo(id) == null) continue;
            JsonElement texture = data.get(name.toLowerCase(Locale.ROOT));
            Property property = texture != null && texture.isJsonObject() ? property(texture.getAsJsonObject()) : null;
            if (property == null) {
                ENTRIES.put(id, new Entry(checked + WITHOUT_SKIN, null));
                continue;
            }
            GameProfile cosmetic = new GameProfile(id, name);
            cosmetic.getProperties().put("textures", property);
            var skin = Minecraft.getInstance().getSkinManager().getOrLoad(cosmetic).exceptionally(error -> null);
            ENTRIES.put(id, new Entry(checked + WITH_SKIN, skin));
        }
    }

    private static Property property(JsonObject texture) {
        JsonElement value = texture.get("value");
        JsonElement signature = texture.get("signature");
        if (value == null || signature == null || !value.isJsonPrimitive() || !signature.isJsonPrimitive()) return null;
        return new Property("textures", value.getAsString(), signature.getAsString());
    }

    public static PlayerSkin forPlayer(PlayerInfo player, PlayerSkin original) {
        Entry entry = ENTRIES.get(player.getProfile().getId());
        if (entry == null || entry.texture() == null) return null;
        PlayerSkin skin = entry.texture().getNow(null);
        if (skin == null || !skin.secure() || skin.textureUrl() == null) return null;
        // La apariencia no cambia la identidad del servidor, la capa, el élitro ni las claves del chat.
        return new PlayerSkin(skin.texture(), skin.textureUrl(), original.capeTexture(), original.elytraTexture(), skin.model(), original.secure());
    }
}
