use color_eyre::eyre;

use crate::discord::Context;

macro_rules! make_getter0 {
    ($fn_name:ident) => {
        // The macro expands into a full function definition
        #[poise::command(slash_command, prefix_command)]
        pub(crate) async fn $fn_name(
            ctx: Context<'_>,
        ) -> eyre::Result<()> {
            let command = format!("/{}", stringify!($fn_name));
            let data = ctx.data();
            data
                .server.read().await
                .send_command(
                    ctx.channel_id(),
                    ctx.author().id,
                    command,
                )
                .await
        }
    };
}

macro_rules! make_getter {
    ($fn_name:ident, $($arg_name:ident : $arg_type:ty),*) => {
        // The macro expands into a full function definition
        #[poise::command(slash_command, prefix_command)]
        pub(crate) async fn $fn_name(
            ctx: Context<'_>,
            $($arg_name: $arg_type),*
        ) -> eyre::Result<()> {
            let mut command = format!("/{}", stringify!($fn_name));
            $(
                command += &format!(" {}", $arg_name);
            )+
            let data = ctx.data();
            data
                .server.read().await
                .send_command(
                    ctx.channel_id(),
                    ctx.author().id,
                    command,
                )
                .await
        }
    };
}
make_getter!(connect, port: u16, baudrate: String);
make_getter0!(disconnect);
make_getter!(print, filename: String);
make_getter0!(files);
make_getter!(unzip, filename:String);
make_getter0!(abort);
make_getter0!(snapshot);
make_getter0!(status);
make_getter0!(help);
make_getter0!(pause);
make_getter0!(resume);
make_getter0!(timelapse);
make_getter0!(mute);
make_getter0!(unmute);
make_getter!(gcode, gcode:String);
make_getter!(getfile, filename:String);
make_getter!(gettimelapse, filename:String);
make_getter0!(poweron);
make_getter0!(poweroff);
make_getter0!(powerstatus);
make_getter0!(listsystemcommands);
make_getter!(systemcommand, command: String);
