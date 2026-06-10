#!/usr/bin/env bash

set -euo pipefail

prog_name="${0##*/}"
im_convert=()

prog_cores=$(($(nproc) / 2))
((prog_cores < 1)) && prog_cores=1

usage() {
	cat <<EOF
Usage: $prog_name <command> [options]


Commands:

  mov2jpg     Convert Apple MOV files to a series of jpgs

  heic2jpg    Convert a directory of HEIC files to a series of jpgs

  fbx2obj     Convert an Autodesk FBX object to an OBJ file (or files)

Options:

  -h, --help  Show this help message

Run '$prog_name <command> --help' for more information on a command.
EOF
}

check_deps() {
	local -a required=("$@")

	declare -A required_tools=(
		[ffmpeg]="ffmpeg"
		[assimp]="assimp"
		[image_magick]="magick convert"
	)

	local optional_tools=(exiftool parallel)

	local missing_req=()
	local missing_opt=()

	for tool in "${required[@]}"; do
		local found=false
		for cmd in ${required_tools["$tool"]}; do
			if command -v "$cmd" >/dev/null 2>&1; then
				found=true
				break
			fi
		done

		if ! $found; then
			missing_req+=("$tool (need one of: ${required_tools[$tool]})")
		fi
	done

	if ((${#missing_req[@]})); then
		echo "Missing required tools:"
		for m in "${missing_req[@]}"; do
			echo "  - $m"
		done
		return 1
	fi

	for cmd in "${optional_tools[@]}"; do
		if ! command -v "$cmd" >/dev/null 2>&1; then
			missing_opt+=("$cmd")
		fi
	done

	if ((${missing_opt[@]})); then
		echo "The following OPTIONAL commands are missing:"
		for cmd in "${missing_opt[@]}"; do
			echo "  - $cmd"
		done
		echo
	fi

	if [[ " ${required[*]} " == *" image_magick "* ]]; then
		if command -v magick >/dev/null 2>&1; then
			im_convert=(magick convert)
		else
			im_convert=(convert)
		fi
	fi

	return 0

}

mov_usage() {
	cat <<EOF
Usage: $prog_name mov2jpg [options] <input> <output_directory>

Options:

  -f <fps>    Set frame rate to extract stills at (default: 3)

  -h          Print this message

EOF
}

cmd_mov2jpg() {
	local fps="3"
	local has_exiftool=false

	if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
		mov_usage
		return 0
	fi

	local opt
	# -f expects an argument, so "f:"; include h for help
	while getopts "f:h" opt; do
		case "$opt" in
		f) fps="$OPTARG" ;;
		h)
			mov_usage
			return 0
			;;
		\?)
			echo "Unknown option: -$OPTARG" >&2
			mov_usage
			return 2
			;;
		esac
	done

	shift $((OPTIND - 1))

	local input="${1:-}"
	local rootout="${2:-}"

	if [[ -z "$input" || -z "$rootout" ]]; then
		echo "Error: <input> and <output_directory> are required." >&2
		mov_usage
		return 2
	fi

	if [[ ! -f "$input" ]]; then
		echo "Error: Input '$input' does not exist."
		mov_usage
		return 2
	fi

	if command -v exiftool >/dev/null 2>&1; then
		has_exiftool=true
	fi

	mkdir -p "$rootout"
	local stem
	stem="$(basename "${input%.*}")"
	local outdir="$rootout/$stem"
	mkdir -p "$outdir"

	ffmpeg -hide_banner -loglevel error \
		-i "$input" \
		-map_metadata 0 \
		-vf "fps=$fps" \
		-vsync 0 \
		-start_number 1 -q:v 1 \
		-color_range mpeg -colorspace bt709 \
		-color_primaries bt709 -color_trc bt709 \
		-f image2 "$outdir/frame_%03d.jpg"

	if $has_exiftool; then
		exiftool -quiet -overwrite_original \
			-ee \
			-TagsFromFile "$input" -all:all -unsafe -icc_profile \
			"$outdir"/frame_*.jpg >/dev/null || true
	fi

	echo "Frames written to $outdir"
	if $has_exiftool; then
		echo "Metadata copied via exiftool where possible"
	fi

	return 0
}

heic_usage() {
	cat <<EOF
Usage: $prog_name heic2jpg <input> [<output>]

If <input> is a directory:
  - If <output> is omitted, an '<input>_jpg' directory is created.

If <input> is a file:
  - If <output> is omitted, '<input>.jpg' is created next to the input.
EOF
}

cmd_heic2jpg() {
	local input_is_directory=false
	local has_parallel=false

	if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
		heic_usage
		return 0
	fi

	local opt
	while getopts "h" opt; do
		case "$opt" in
		h)
			heic_usage
			return 0
			;;
		\?)
			echo "Unknown option: -$OPTARG" >&2
			heic_usage
			return 2
			;;
		esac
	done
	shift $((OPTIND - 1))

	local input="${1:-}"
	local output="${2:-}"

	if [[ -z "$input" ]]; then
		echo "Error: <input> is required." >&2
		heic_usage
		return 2
	fi

	if [[ ! -e "$input" ]]; then
		echo "Error: Input '$input' does not exist."
		heic_usage
		return 2
	fi

	if [[ -d "$input" ]]; then
		input_is_directory=true
	fi

	# Derive output if not provided
	if [[ -z "$output" ]]; then
		if $input_is_directory; then
			output="${input%/}_jpg"
		else
			output="${input%.*}.jpg"
		fi
	fi

	if command -v parallel >/dev/null 2>&1; then
		has_parallel=true
	fi

	if $input_is_directory; then
		mkdir -p "$output"
		if $has_parallel; then
			find "$input" -type f -iname "*.heic" |
				parallel --bar -I{} -j"$prog_cores" \
					"${im_convert[@]}" "{}" -quality 95 "$output/{/.}.jpg"
		else
			while IFS= read -r i; do
				local base out
				base="$(basename "$i")"
				out="$output/${base%.*}.jpg"
				"${im_convert[@]}" "$i" -quality 95 "$out"
			done < <(find "$input" -type f -iname "*.heic")
		fi
	else
		"${im_convert[@]}" "$input" -quality 95 "$output"
	fi

	return 0
}

fbx_usage() {
	cat <<EOF
Usage: $prog_name fbx2obj <input> [<output>]

If <input> is a directory:
  - All *.fbx files inside are converted.
  - If <output> is omitted, an '<input>_obj' directory is created.

If <input> is a file:
  - If <output> is omitted, '<input>.obj' is created next to the input.
EOF
}

cmd_fbx2obj() {
	local input_is_directory=false
	local has_parallel=false

	if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
		fbx_usage
		return 0
	fi

	local opt
	while getopts "h" opt; do
		case "$opt" in
		h)
			fbx_usage
			return 0
			;;
		\?)
			echo "Unknown option: -$OPTARG" >&2
			fbx_usage
			return 2
			;;
		esac
	done
	shift $((OPTIND - 1))

	local input="${1:-}"
	local output="${2:-}"

	if [[ -z "$input" ]]; then
		echo "Error: <input> is required." >&2
		fbx_usage
		return 2
	fi

	if [[ ! -e "$input" ]]; then
		echo "Error: Input '$input' does not exist."
		fbx_usage
		return 2
	fi

	if [[ -d "$input" ]]; then
		input_is_directory=true
	fi

	# Derive output if not provided
	if [[ -z "$output" ]]; then
		if $input_is_directory; then
			output="${input%/}_obj"
		else
			output="${input%.*}.obj"
		fi
	fi

	if command -v parallel >/dev/null 2>&1; then
		has_parallel=true
	fi

	if $input_is_directory; then
		mkdir -p "$output"
		if $has_parallel; then
			find "$input" -type f -iname "*.fbx" |
				parallel --bar -I{} -j"$prog_cores" \
					assimp export "{}" "$output/{/.}.obj"
		else
			while IFS= read -r f; do
				local base out
				base="$(basename "$f")"
				out="$output/${base%.*}.obj"
				assimp export "$f" "$out"
			done < <(find "$input" -type f -iname "*.fbx")
		fi
	else
		assimp export "$input" "$output"
	fi

	return 0
}

main() {
	if [[ $# -eq 0 ]]; then
		usage
		exit 1
	fi

	case "$1" in
	-h | --help)
		usage
		exit 0
		;;
	mov2jpg)
		shift
		check_deps ffmpeg || exit 1
		cmd_mov2jpg "$@"
		;;
	heic2jpg)
		shift
		check_deps image_magick || exit 1
		cmd_heic2jpg "$@"
		;;
	fbx2obj)
		shift
		check_deps assimp || exit 1
		cmd_fbx2obj "$@"
		;;
	*)
		echo "Unknown command: $1" >&2
		usage
		exit 1
		;;
	esac
}

main "$@"
