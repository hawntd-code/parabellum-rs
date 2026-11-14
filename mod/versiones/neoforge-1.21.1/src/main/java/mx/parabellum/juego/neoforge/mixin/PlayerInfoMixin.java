package mx.parabellum.juego.neoforge.mixin;

import mx.parabellum.juego.neoforge.SkinsRemotas;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.resources.PlayerSkin;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(PlayerInfo.class)
public abstract class PlayerInfoMixin {
    @Inject(method = "getSkin", at = @At("RETURN"), cancellable = true)
    private void parabellum$skin(CallbackInfoReturnable<PlayerSkin> callback) {
        PlayerSkin skin = SkinsRemotas.forPlayer((PlayerInfo) (Object) this, callback.getReturnValue());
        if (skin != null) callback.setReturnValue(skin);
    }
}
